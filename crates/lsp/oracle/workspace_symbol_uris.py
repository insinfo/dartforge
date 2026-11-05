"""Compare package URI boundaries, escaped paths and generated replacements.

Usage: ORACLE_DART=... ORACLE_SDK_LIB=... python3 workspace_symbol_uris.py
       <output.jsonl> <dartforge-lsp> [--fixture <path>] [--without-watcher]
       [--source-shadow]

External leaves expose which package imports resolved before the first query.
Decoys outside lib expose incorrect filesystem traversal across package roots.
--source-shadow also checks a non-lib source whose URI selects a generated file,
first unreferenced and then imported through an unsaved edit.
Every full ordered JSON difference is retained and causes exit status 1.
"""

import argparse
import copy
import json
import os
from pathlib import Path
import tempfile
import time

from oracle import CAPABILITIES, DART, DART_CACHE, SDK_LIB, Lsp


def _write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _run(output, binary, fixture, watch_files, source_shadow=False):
    root = fixture / "workspace"
    dependency = fixture / "dependency"
    anchor = root / "lib/anchor.dart"
    files = [
        ("root_target.dart", "Root"),
        ("slash_target.dart", "Slash"),
        ("space absolute.dart", "Space"),
        ("dir/encoded.dart", "EncodedSlash"),
        ("query.dart", "Query"),
        ("fragment.dart", "Fragment"),
        ("café.dart", "Unicode"),
        ("generated_target.dart", "GeneratedTarget"),
    ]
    for path, label in files:
        leaf = fixture / "leaves" / f"{label}.dart"
        _write(leaf, f"class UriResolveProbeLeaf{label} {{}}\n")
        _write(dependency / "lib" / path,
               f"import '{leaf.as_uri()}'; class UriResolveProbeTarget{label} {{}}\n")
    _write(dependency / "root_target.dart", "class UriResolveProbeOutsideRootDecoy {}\n")
    _write(dependency / "generated_target.dart", "class UriResolveProbeOutsideGeneratedDecoy {}\n")
    _write(dependency / "lib/src/entry.dart",
           "import '../../root_target.dart';\n"
           "import '/slash_target.dart';\n"
           "import 'package:uri_dependency/space%20absolute.dart';\n"
           "import '../dir%2Fencoded.dart';\n"
           "import 'package:uri_dependency/query.dart?mode=1';\n"
           "import 'package:uri_dependency/fragment.dart#tag';\n"
           "import 'package:uri_dependency/caf%C3%A9.dart';\n"
           "class UriResolveProbeDependency {}\n")
    _write(root / ".dart_tool/build/generated/uri_dependency/lib/generated.dart",
           "import '../../generated_target.dart'; class UriResolveProbeGenerated {}\n")
    # The SDK's generated fallback uses the encoded URI path verbatim. This
    # physical space-named output must not satisfy generated%20file.dart.
    _write(root / ".dart_tool/build/generated/uri_dependency/lib/generated file.dart",
           "class UriResolveProbeGeneratedSpace {}\n")
    _write(dependency / "pubspec.yaml", 'name: uri_dependency\nenvironment:\n  sdk: ">=3.6.2 <4.0.0"\n')
    _write(root / "pubspec.yaml", 'name: uri_workspace\nenvironment:\n  sdk: ">=3.6.2 <4.0.0"\n')
    _write(root / ".dart_tool/package_config.json", json.dumps({"configVersion": 2, "packages": [
        {"name": "uri_workspace", "rootUri": "../", "packageUri": "lib/", "languageVersion": "3.6"},
        {"name": "uri_dependency", "rootUri": dependency.as_uri() + "/", "packageUri": "lib/", "languageVersion": "3.6"},
    ]}))
    source = ("import 'package:uri_dependency/src/entry.dart';\n"
              "import 'package:uri_dependency/generated.dart';\n"
              "import 'package:uri_dependency/generated%20file.dart';\nclass UriResolveProbeEntry {}\n")
    _write(anchor, source)
    shadow = root / "test/shadow.dart"
    if source_shadow:
        _write(shadow, "class UriResolveProbeSourceShadow {}\n")
        _write(root / ".dart_tool/build/generated/uri_workspace/test/shadow.dart",
               "class UriResolveProbeBuiltShadow {}\n")
    capabilities = copy.deepcopy(CAPABILITIES)
    if watch_files:
        capabilities["workspace"]["didChangeWatchedFiles"] = {"dynamicRegistration": True}
    command = [DART, "language-server", "--protocol=lsp", "--client-id=oracle"]
    if DART_CACHE:
        command.append("--cache=" + DART_CACHE)
    servers = []
    total = mismatches = 0

    def record(phase):
        nonlocal total, mismatches
        for index, query in enumerate(("UriResolveProbe", "UriResolveProbeLeaf", "UriResolveProbe", "UriResolveProbe")):
            row = {"project": "symbol_uris", "phase": phase, "method": "workspace/symbol",
                   "pos": f"{index}:{query}", "watchedFiles": watch_files, "sourceShadow": source_shadow}
            for server in servers:
                row[server.name], _ = server.request("workspace/symbol", {"query": query})
                if not isinstance(row[server.name], list):
                    raise RuntimeError(f"{phase} {query} {server.name}: {row[server.name]}")
            same = row["dart"] == row["dartforge"]
            output.write(json.dumps(row, ensure_ascii=False) + "\n")
            output.flush()
            total += 1
            mismatches += not same
            print(f"{phase} {index}:{query}: {'equal' if same else 'DIFFERENT'} "
                  f"({len(row['dart'])}/{len(row['dartforge'])})", flush=True)
            names = [s["name"] for s in row["dart"]]
            if phase == "closed":
                assert not names, names
            else:
                assert not any("Decoy" in n for n in names), names
                assert "UriResolveProbeSourceShadow" not in names, names
                assert "UriResolveProbeGeneratedSpace" not in names, names
                for _, label in files:
                    if phase == "initial" and index == 0 and label == "Fragment":
                        assert f"UriResolveProbeLeaf{label}" not in names, names
                        continue
                    assert f"UriResolveProbeLeaf{label}" in names, (phase, index, label, names)
                if index != 1:
                    assert "UriResolveProbeGenerated" in names, names
                    if source_shadow:
                        assert ("UriResolveProbeBuiltShadow" in names) == (phase == "referenced"), names
                    if phase == "referenced":
                        assert "UriResolveProbeRevision" in names, names

    try:
        servers = [Lsp("dart", command), Lsp("dartforge", [binary, "--stdio"],
                    env=dict(os.environ, DARTFORGE_SDK_LIB=SDK_LIB))]
        for server in servers:
            response, _ = server.request("initialize", {"processId": os.getpid(), "rootUri": root.as_uri(),
                "workspaceFolders": [{"uri": root.as_uri(), "name": root.name}], "capabilities": capabilities,
                "initializationOptions": {"onlyAnalyzeProjectsWithOpenFiles": True}})
            if not isinstance(response, dict) or "capabilities" not in response:
                raise RuntimeError(f"initialize failed: {server.name}: {response}")
            server.notify("initialized", {})
        previous = servers[0].analysis_completions
        for server in servers:
            server.notify("textDocument/didOpen", {"textDocument": {
                "uri": anchor.as_uri(), "languageId": "dart", "version": 1, "text": source}})
        if not servers[0].wait_analysis(previous, limit=90, quiet=1.0):
            raise RuntimeError("initial analysis did not finish")
        record("initial")
        record("repeat")
        if source_shadow:
            previous = servers[0].analysis_completions
            text = f"import '{shadow.as_uri()}';\n" + source + "class UriResolveProbeRevision {}\n"
            for server in servers:
                server.notify("textDocument/didChange", {"textDocument": {"uri": anchor.as_uri(), "version": 2},
                                                       "contentChanges": [{"text": text}]})
            if not servers[0].wait_analysis(previous, limit=90, quiet=1.0):
                raise RuntimeError("analysis after importing the shadowed file did not finish")
            record("referenced")
        for server in servers:
            server.notify("textDocument/didClose", {"textDocument": {"uri": anchor.as_uri()}})
        deadline = time.monotonic() + 30
        while True:
            params = {"textDocument": {"uri": anchor.as_uri()}}
            probes = [server.request("textDocument/documentSymbol", params)[0] for server in servers]
            if isinstance(probes[0], dict) and probes[0].get("error", {}).get("code") == -32007:
                break
            if time.monotonic() >= deadline:
                raise RuntimeError(f"last context was not closed: {probes[0]}")
            time.sleep(0.1)
        record("closed")
    finally:
        for server in servers:
            server.close()
    print(f"complete ordered responses: {total - mismatches}/{total}", flush=True)
    return bool(mismatches)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output")
    parser.add_argument("binary")
    parser.add_argument("--fixture", type=Path)
    parser.add_argument("--without-watcher", action="store_true")
    parser.add_argument("--source-shadow", action="store_true",
                        help="also test a real non-lib source shadowed by a generated output")
    args = parser.parse_args()

    def run(fixture):
        fixture.mkdir(parents=True, exist_ok=True)
        marker = fixture / "uri_fixture.json"
        identity = {"oracle": "workspace_symbol_uris", "version": 1}
        if marker.exists():
            if json.loads(marker.read_text(encoding="utf-8")) != identity:
                raise RuntimeError(f"unsupported fixture: {fixture}")
        elif any(fixture.iterdir()):
            raise RuntimeError(f"fixture must be empty or oracle-owned: {fixture}")
        else:
            marker.write_text(json.dumps(identity), encoding="utf-8")
        with open(args.output, "w", encoding="utf-8") as output:
            return _run(output, os.path.abspath(args.binary), fixture, not args.without_watcher, args.source_shadow)

    if args.fixture:
        return run(args.fixture.resolve())
    with tempfile.TemporaryDirectory(prefix="dartforge-symbol-uris-") as temporary:
        return run(Path(temporary).resolve())


if __name__ == "__main__":
    raise SystemExit(main())

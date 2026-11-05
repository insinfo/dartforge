"""Compare symbol type aliases through overlays and package catalog replacement.

Usage: ORACLE_DART=... ORACLE_SDK_LIB=... python3 workspace_symbol_aliases.py
       <output.jsonl> <dartforge-lsp> [--fixture <path>] [--without-watcher]

Return aliases affect the executable's displayed suffix. Keep complete ordered
responses through body edits, alias changes, equal and different replacement
catalogs, dependency removal and closing the last document. Exit 1 on mismatch.
"""

import argparse
import copy
import json
import os
from pathlib import Path
import tempfile
import time

from oracle import CAPABILITIES, DART, DART_CACHE, SDK_LIB, Lsp, settle_initial_analysis


def run(output, binary, fixture, watch_files):
    root = fixture / "workspace"
    anchor = root / "lib/entry.dart"
    anchor.parent.mkdir(parents=True, exist_ok=True)
    config = root / ".dart_tool/package_config.json"
    config.parent.mkdir(parents=True, exist_ok=True)
    versions = [fixture / name for name in ("one", "two", "three")]
    for version, target in zip(versions, ("(int, int)", "int", "int")):
        (version / "lib").mkdir(parents=True, exist_ok=True)
        (version / "pubspec.yaml").write_text('name: alias_dependency\nenvironment:\n  sdk: ">=3.6.2 <4.0.0"\n', encoding="utf-8")
        (version / "lib/alias.dart").write_text(f"typedef AliasMemoProbeType = {target};\n", encoding="utf-8")
    (root / "pubspec.yaml").write_text('name: alias_workspace\nenvironment:\n  sdk: ">=3.6.2 <4.0.0"\n', encoding="utf-8")
    source = ("import 'package:alias_dependency/alias.dart';\n"
              "AliasMemoProbeType AliasMemoProbeFunction() { throw 0; }\n")
    anchor.write_text(source, encoding="utf-8")

    def catalog(version):
        packages = [{"name": "alias_workspace", "rootUri": "../", "packageUri": "lib/", "languageVersion": "3.6"}]
        if version is not None:
            packages.append({"name": "alias_dependency", "rootUri": version.as_uri() + "/", "packageUri": "lib/", "languageVersion": "3.6"})
        config.write_text(json.dumps({"configVersion": 2, "packages": packages}), encoding="utf-8")

    catalog(versions[0])
    capabilities = copy.deepcopy(CAPABILITIES)
    if watch_files:
        capabilities["workspace"]["didChangeWatchedFiles"] = {"dynamicRegistration": True}
    command = [DART, "language-server", "--protocol=lsp", "--client-id=oracle"]
    if DART_CACHE:
        command.append("--cache=" + DART_CACHE)
    servers = []
    total = mismatches = 0

    def ready(previous):
        if not servers[0].wait_analysis(previous, limit=90, quiet=1):
            raise RuntimeError("analysis after the alias change did not finish")
        settle_initial_analysis(servers, [anchor])

    def record(phase, closed=False):
        nonlocal total, mismatches
        for index, query in enumerate(("AliasMemoProbeFunction", "AliasMemoProbe", "AliasMemoProbeFunction")):
            row = {"project": "symbol_aliases", "phase": phase, "method": "workspace/symbol",
                   "pos": f"{index}:{query}", "watchedFiles": watch_files}
            for server in servers:
                row[server.name], _ = server.request("workspace/symbol", {"query": query})
                if not isinstance(row[server.name], list):
                    raise RuntimeError(f"{phase} {server.name}: {row[server.name]}")
            same = row["dart"] == row["dartforge"]
            output.write(json.dumps(row, ensure_ascii=False) + "\n")
            output.flush()
            total += 1
            mismatches += not same
            names = [s["name"] for s in row["dart"]]
            if closed:
                assert not names, names
            else:
                assert any(n.startswith("AliasMemoProbeFunction") for n in names), names
            print(f"{phase} {index}: {'equal' if same else 'DIFFERENT'} {names}", flush=True)

    try:
        servers = [Lsp("dart", command), Lsp("dartforge", [binary, "--stdio"],
                   env=dict(os.environ, DARTFORGE_SDK_LIB=SDK_LIB))]
        for server in servers:
            result, _ = server.request("initialize", {"processId": os.getpid(), "rootUri": root.as_uri(),
                "workspaceFolders": [{"uri": root.as_uri(), "name": root.name}], "capabilities": capabilities,
                "initializationOptions": {"onlyAnalyzeProjectsWithOpenFiles": True}})
            if not isinstance(result, dict) or "capabilities" not in result:
                raise RuntimeError(f"initialize failed: {server.name}: {result}")
            server.notify("initialized", {})
        previous = servers[0].analysis_completions
        for server in servers:
            server.notify("textDocument/didOpen", {"textDocument": {
                "uri": anchor.as_uri(), "languageId": "dart", "version": 1, "text": source}})
        ready(previous)
        record("initial")
        record("repeat")
        previous = servers[0].analysis_completions
        for server in servers:
            server.notify("textDocument/didChange", {"textDocument": {"uri": anchor.as_uri(), "version": 2},
                "contentChanges": [{"text": source.replace("throw 0;", "final local = (3, 4); return local;")}]})
        ready(previous)
        record("body")
        alias = versions[0] / "lib/alias.dart"
        previous = servers[0].analysis_completions
        for server in servers:
            server.notify("textDocument/didOpen", {"textDocument": {"uri": alias.as_uri(), "languageId": "dart", "version": 1,
                "text": "typedef AliasMemoProbeType = void Function();\n"}})
        ready(previous)
        record("alias_open")
        previous = servers[0].analysis_completions
        for server in servers:
            server.notify("textDocument/didClose", {"textDocument": {"uri": alias.as_uri()}})
        ready(previous)
        record("alias_closed")
        for phase, version in (("catalog_changed", versions[1]), ("catalog_equal", versions[2])):
            previous = servers[0].analysis_completions
            catalog(version)
            if watch_files:
                for server in servers:
                    server.notify("workspace/didChangeWatchedFiles", {"changes": [{"uri": config.as_uri(), "type": 2}]})
            ready(previous)
            record(phase)
        previous = servers[0].analysis_completions
        for server in servers:
            server.notify("textDocument/didChange", {"textDocument": {"uri": anchor.as_uri(), "version": 3},
                "contentChanges": [{"text": "int AliasMemoProbeFunction() => 0;\n"}]})
        ready(previous)
        previous = servers[0].analysis_completions
        catalog(None)
        if watch_files:
            for server in servers:
                server.notify("workspace/didChangeWatchedFiles", {"changes": [{"uri": config.as_uri(), "type": 2}]})
        ready(previous)
        record("dependency_removed")
        for server in servers:
            server.notify("textDocument/didClose", {"textDocument": {"uri": anchor.as_uri()}})
        deadline = time.monotonic() + 30
        while True:
            probe, _ = servers[0].request("textDocument/documentSymbol", {"textDocument": {"uri": anchor.as_uri()}})
            if isinstance(probe, dict) and probe.get("error", {}).get("code") == -32007:
                break
            if time.monotonic() >= deadline:
                raise RuntimeError(f"last context was not closed: {probe}")
            time.sleep(0.1)
        record("closed", closed=True)
    finally:
        for server in servers:
            server.close()
    print(f"complete ordered responses: {total - mismatches}/{total}", flush=True)
    return bool(mismatches)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("binary")
    parser.add_argument("--fixture", type=Path)
    parser.add_argument("--without-watcher", action="store_true")
    args = parser.parse_args()

    def execute(fixture):
        fixture.mkdir(parents=True, exist_ok=True)
        marker = fixture / "alias_fixture.json"
        if marker.exists():
            if json.loads(marker.read_text(encoding="utf-8")) != {"version": 1}:
                raise RuntimeError(f"unsupported fixture: {fixture}")
        elif any(fixture.iterdir()):
            raise RuntimeError(f"fixture must be empty or oracle-owned: {fixture}")
        marker.write_text(json.dumps({"version": 1}), encoding="utf-8")
        with args.output.open("w", encoding="utf-8") as output:
            return run(output, os.path.abspath(args.binary), fixture, not args.without_watcher)

    if args.fixture:
        return execute(args.fixture.resolve())
    with tempfile.TemporaryDirectory(prefix="dartforge-symbol-aliases-") as temporary:
        return execute(Path(temporary).resolve())


if __name__ == "__main__":
    raise SystemExit(main())

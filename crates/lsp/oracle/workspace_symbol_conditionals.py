"""Compare conditional URI discovery, query snapshots and unsaved directives.

Usage: ORACLE_DART=... ORACLE_SDK_LIB=... python3 workspace_symbol_conditionals.py
       <output.jsonl> <dartforge-lsp> [--fixture <path>]

Alternative files live outside the workspace and package catalogs. Each imports
another external file, exposing declarations discovered while resolving a query.
Every ordered JSON difference is retained and produces exit status 1.
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


def _run(output, binary, fixture):
    root = fixture / "workspace"
    anchor = root / "lib/anchor.dart"
    labels = ["ImportBase", "ImportIo", "ImportFalse", "ImportEmpty", "ImportCustom",
              "ExportBase", "ExportIo", "ExportFalse", "Replacement"]
    for label in labels:
        _write(fixture / "external" / f"{label}.dart",
               f"import '../leaves/{label}.dart';\nclass UriBranchProbeBranch{label} {{}}\n")
        _write(fixture / "leaves" / f"{label}.dart", f"class UriBranchProbeNested{label} {{}}\n")
    _write(root / "pubspec.yaml", 'name: conditional_workspace\nenvironment:\n  sdk: ">=3.6.2 <4.0.0"\n')
    _write(root / ".dart_tool/package_config.json", json.dumps({"configVersion": 2, "packages": [
        {"name": "conditional_workspace", "rootUri": "../", "packageUri": "lib/", "languageVersion": "3.6"},
    ]}))
    def uri(label):
        return (fixture / "external" / f"{label}.dart").as_uri()

    source = (f"import '{uri('ImportBase')}'\n"
              f"  if (dart.library.io) '{uri('ImportIo')}'\n"
              f"  if (dart.library.io == 'false') '{uri('ImportFalse')}'\n"
              f"  if (custom.setting == '') '{uri('ImportEmpty')}'\n"
              f"  if (custom.setting == 'chosen') '{uri('ImportCustom')}';\n"
              f"export '{uri('ExportBase')}'\n"
              f"  if (dart.library.html) '{uri('ExportIo')}'\n"
              f"  if (dart.library.html == 'false') '{uri('ExportFalse')}';\n"
              "class UriBranchProbeEntry {}\n")
    _write(anchor, source)
    capabilities = copy.deepcopy(CAPABILITIES)
    capabilities["workspace"]["didChangeWatchedFiles"] = {"dynamicRegistration": True}
    command = [DART, "language-server", "--protocol=lsp", "--client-id=oracle"]
    if DART_CACHE:
        command.append("--cache=" + DART_CACHE)
    servers = []
    total = mismatches = 0

    def record(phase):
        nonlocal total, mismatches
        for index, query in enumerate(("UriBranchProbe", "UriBranchProbeNested", "UriBranchProbe", "UriBranchProbe")):
            row = {"project": "symbol_conditionals", "phase": phase,
                   "method": "workspace/symbol", "pos": f"{index}:{query}"}
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
            if index == 0 and phase in ("unsaved", "reverted"):
                revision = "Two" if phase == "unsaved" else "Three"
                assert f"UriBranchProbeRevision{revision}" in names, names
            if phase == "initial" and index == 0:
                assert "UriBranchProbeBranchImportIo" in names, names
                assert "UriBranchProbeBranchImportEmpty" in names, names
                assert "UriBranchProbeNestedImportBase" in names, names
                assert "UriBranchProbeNestedImportIo" not in names, names
                assert "UriBranchProbeNestedImportFalse" not in names, names
                assert "UriBranchProbeNestedImportEmpty" not in names, names
            if phase == "initial" and index == 1:
                assert "UriBranchProbeNestedImportIo" in names, names
                assert "UriBranchProbeNestedImportFalse" in names, names
                assert "UriBranchProbeNestedImportEmpty" in names, names
            if phase == "closed":
                assert not names, names

    try:
        servers = [Lsp("dart", command), Lsp("dartforge", [binary, "--stdio"],
                    env=dict(os.environ, DARTFORGE_SDK_LIB=SDK_LIB))]
        for server in servers:
            response, _ = server.request("initialize", {"processId": os.getpid(), "rootUri": root.as_uri(),
                "workspaceFolders": [{"uri": root.as_uri(), "name": root.name}],
                "capabilities": capabilities,
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
        for version, phase, text in (
                (2, "unsaved", source.replace("ImportFalse.dart", "Replacement.dart") + "class UriBranchProbeRevisionTwo {}\n"),
                (3, "reverted", source + "class UriBranchProbeRevisionThree {}\n")):
            previous = servers[0].analysis_completions
            for server in servers:
                server.notify("textDocument/didChange", {"textDocument": {"uri": anchor.as_uri(), "version": version},
                                                       "contentChanges": [{"text": text}]})
            if not servers[0].wait_analysis(previous, limit=90, quiet=1.0):
                raise RuntimeError(f"{phase} analysis did not finish")
            record(phase)
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
    parser.add_argument("--fixture", type=Path, help="keep and reuse this oracle's fixture directory")
    args = parser.parse_args()

    def run(fixture):
        fixture.mkdir(parents=True, exist_ok=True)
        marker = fixture / "conditional_fixture.json"
        identity = {"oracle": "workspace_symbol_conditionals", "version": 1}
        if marker.exists():
            if json.loads(marker.read_text(encoding="utf-8")) != identity:
                raise RuntimeError(f"unsupported fixture: {fixture}")
        elif any(fixture.iterdir()):
            raise RuntimeError(f"fixture must be empty or oracle-owned: {fixture}")
        else:
            marker.write_text(json.dumps(identity), encoding="utf-8")
        with open(args.output, "w", encoding="utf-8") as output:
            return _run(output, os.path.abspath(args.binary), fixture)

    if args.fixture:
        return run(args.fixture.resolve())
    with tempfile.TemporaryDirectory(prefix="dartforge-symbol-conditionals-") as temporary:
        return run(Path(temporary).resolve())


if __name__ == "__main__":
    raise SystemExit(main())

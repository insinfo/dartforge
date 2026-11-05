"""Compare workspace symbols when a missing part appears in memory or on disk.

Usage: ORACLE_DART=... ORACLE_SDK_LIB=... python3 workspace_symbol_parts.py
       <output.jsonl> <dartforge-lsp> [--without-watcher] [--fixture <path>]

The owner's text never changes. The part is opened before it exists on disk,
closed, created, edited, deleted and recreated. Full ordered JSON differences
are retained and cause exit status 1. This checks correctness, not performance.
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
    anchor = root / "lib/owner.dart"
    part = root / "lib/generated.g.dart"
    anchor.parent.mkdir(parents=True, exist_ok=True)
    part.unlink(missing_ok=True)
    source = "part 'generated.g.dart'; class PartRefreshProbeOwner {}\n"
    anchor.write_text(source, encoding="utf-8")
    (root / "pubspec.yaml").write_text('name: part_refresh\nenvironment:\n  sdk: ">=3.6.2 <4.0.0"\n', encoding="utf-8")
    config = root / ".dart_tool/package_config.json"
    config.parent.mkdir(parents=True, exist_ok=True)
    config.write_text(json.dumps({"configVersion": 2, "packages": [
        {"name": "part_refresh", "rootUri": "../", "packageUri": "lib/", "languageVersion": "3.6"}
    ]}), encoding="utf-8")
    capabilities = copy.deepcopy(CAPABILITIES)
    if watch_files:
        capabilities["workspace"]["didChangeWatchedFiles"] = {"dynamicRegistration": True}
    command = [DART, "language-server", "--protocol=lsp", "--client-id=oracle"]
    if DART_CACHE:
        command.append("--cache=" + DART_CACHE)
    servers = []
    total = mismatches = 0

    def part_text(revision):
        return (f"part of 'owner.dart'; class PartRefreshProbeGenerated{revision} {{\n"
                f"  PartRefreshProbeGenerated{revision}.PartRefreshProbeCtor();\n"
                "  int PartRefreshProbeField = 1;\n"
                "  void PartRefreshProbeMethod() {}\n}\n")

    def ready(previous):
        if not servers[0].wait_analysis(previous, limit=90, quiet=1):
            raise RuntimeError("analysis after changing the part did not finish")
        settle_initial_analysis(servers, [anchor])

    def record(phase, revision=None, closed=False):
        nonlocal total, mismatches
        for index, query in enumerate(("PartRefreshProbe", "PartRefreshProbeGenerated", "PartRefreshProbe")):
            row = {"project": "symbol_parts", "phase": phase, "method": "workspace/symbol",
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
            generated = f"PartRefreshProbeGenerated{revision}" if revision else None
            if closed:
                assert not names, names
            elif index != 1:
                assert "PartRefreshProbeOwner" in names, names
            if generated:
                assert generated in names, names
            else:
                assert not any(n.startswith("PartRefreshProbeGenerated") for n in names), names
            print(f"{phase} {index}: {'equal' if same else 'DIFFERENT'} "
                  f"({len(row['dart'])}/{len(row['dartforge'])})", flush=True)

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
        record("missing")
        record("repeat_missing")
        previous = servers[0].analysis_completions
        for server in servers:
            server.notify("textDocument/didOpen", {"textDocument": {
                "uri": part.as_uri(), "languageId": "dart", "version": 1, "text": part_text("Overlay")}})
        ready(previous)
        record("opened_missing", "Overlay")
        previous = servers[0].analysis_completions
        for server in servers:
            server.notify("textDocument/didClose", {"textDocument": {"uri": part.as_uri()}})
        ready(previous)
        record("closed_missing")
        for phase, revision, change in (("created", "DiskOne", 1), ("edited", "DiskEdited", 2),
                                         ("deleted", None, 3), ("recreated", "DiskTwo", 1)):
            previous = servers[0].analysis_completions
            if revision:
                part.write_text(part_text(revision), encoding="utf-8")
            else:
                part.unlink()
            if watch_files:
                for server in servers:
                    server.notify("workspace/didChangeWatchedFiles", {"changes": [
                        {"uri": part.as_uri(), "type": change}]})
            ready(previous)
            record(phase, revision)
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
    parser.add_argument("--without-watcher", action="store_true")
    parser.add_argument("--fixture", type=Path)
    args = parser.parse_args()

    def execute(fixture):
        fixture.mkdir(parents=True, exist_ok=True)
        marker = fixture / "parts_fixture.json"
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
    with tempfile.TemporaryDirectory(prefix="dartforge-symbol-parts-") as temporary:
        return execute(Path(temporary).resolve())


if __name__ == "__main__":
    raise SystemExit(main())

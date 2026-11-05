"""Compare workspace symbols through driver-map growth, rebuild and removal.

Usage: ORACLE_DART=... ORACLE_SDK_LIB=... python3 workspace_symbol_capacity.py
       <output.jsonl> <dartforge-lsp> [--query-after-reduction] [--fixture <path>]

Only seven projects are analyzed. A small program run by the reference VM
selects paths whose HashMap order exposes both resize history and rebuilding
an already grown table. Dependencies are unimported, so their symbol order
comes from discoverAvailableFiles rather than analysis scheduling.
Exit status is 1 on any complete ordered response mismatch.
"""

import argparse
import copy
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

from oracle import CAPABILITIES, DART, DART_CACHE, SDK_LIB, Lsp


_SELECT_PATHS = r"""
import 'dart:collection';
import 'dart:convert';

void main(List<String> paths) {
  List<int> creation(List<int> ids) {
    final roots = HashSet<String>.from(ids.map((i) => paths[i])).toList();
    roots.sort((a, b) => a.length - b.length);
    return roots.map(paths.indexOf).toList();
  }
  List<int> order(List<int> ids, {bool grown = false}) {
    final map = HashMap<String, int>();
    if (grown) {
      for (var i = 0; i < 7; i++) { map[paths[i]] = i; }
      for (var i = 0; i < 7; i++) { map.remove(paths[i]); }
    }
    for (final i in creation(ids)) { map[paths[i]] = i; }
    return map.values.toList();
  }
  for (var a = 0; a < paths.length; a++) {
    for (var b = a + 1; b < paths.length; b++) {
      final pair = [a, b];
      if (jsonEncode(order(pair)) == jsonEncode(order(pair, grown: true))) continue;
      for (var c = 0; c < paths.length; c++) {
        for (var d = c + 1; d < paths.length; d++) {
          if ((paths[c].hashCode & 15) != (paths[d].hashCode & 15)) continue;
          final peak = <int>{a, b, c, d};
          for (var i = 0; peak.length < 7; i++) { peak.add(i); }
          final ids = peak.toList()..sort();
          final initial = order(ids);
          final rebuilt = order(ids, grown: true);
          if (jsonEncode(initial) == jsonEncode(rebuilt)) continue;
          print(jsonEncode({'peak': ids, 'survivors': pair,
            'contextOrder': creation(ids), 'survivorContextOrder': creation(pair),
            'initialOrder': initial, 'rebuiltOrder': rebuilt,
            'freshPairOrder': order(pair), 'retainedPairOrder': order(pair, grown: true)}));
          return;
        }
      }
    }
  }
  throw StateError('could not select paths exposing HashMap history');
}
"""


def _write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _prepare_fixture(fixture):
    """Reuse only an oracle-owned directory and reset its known phase options.

    Keeping absolute paths stable matters: Folder.hashCode changes the order
    being tested. Unknown directories and edited options are never removed.
    """
    fixture.mkdir(parents=True, exist_ok=True)
    marker = fixture / "capacity_fixture.json"
    identity = {"oracle": "workspace_symbol_capacity", "version": 1}
    if marker.exists():
        if json.loads(marker.read_text(encoding="utf-8")) != identity:
            raise RuntimeError(f"unsupported capacity fixture: {fixture}")
    elif any(fixture.iterdir()):
        raise RuntimeError(f"capacity fixture must be empty or oracle-owned: {fixture}")
    else:
        marker.write_text(json.dumps(identity), encoding="utf-8")
    for options in fixture.glob("project_*/analysis_options.yaml"):
        if options.read_text(encoding="utf-8") not in (
                "# Driver capacity scenario: expanded\n", "# Driver capacity scenario: small\n"):
            raise RuntimeError(f"unexpected edited fixture options: {options}")
        options.unlink()


def _run(output, binary, fixture, query_after_reduction):
    projects = [fixture / f"project_{i:02}" for i in range(24)]
    for i, root in enumerate(projects):
        package = f"capacity_project_{i:02}"
        dependency = fixture / "dependencies" / f"dependency_{i:02}"
        _write(root / "pubspec.yaml", f'name: {package}\nenvironment:\n  sdk: ">=3.6.2 <4.0.0"\n')
        _write(root / "lib/entry.dart", f"class CapacityEntry{i:02} {{}}\n")
        _write(dependency / "lib/api.dart", f"class CapacityDependency{i:02} {{}}\n")
        _write(root / ".dart_tool/package_config.json", json.dumps({"configVersion": 2, "packages": [
            {"name": package, "rootUri": "../", "packageUri": "lib/", "languageVersion": "3.6"},
            {"name": f"capacity_dependency_{i:02}", "rootUri": dependency.as_uri() + "/",
             "packageUri": "lib/", "languageVersion": "3.6"},
        ]}))
    selector = fixture / "select_paths.dart"
    selector.write_text(_SELECT_PATHS, encoding="utf-8")
    case = json.loads(subprocess.check_output([DART, str(selector), *map(str, projects)], text=True))
    assert len(case["peak"]) == 7 and len(case["survivors"]) == 2, case
    assert case["initialOrder"] != case["rebuiltOrder"], case
    assert case["freshPairOrder"] != case["retainedPairOrder"], case
    case["roots"] = {str(i): str(projects[i]) for i in case["peak"]}
    print(json.dumps(case), flush=True)
    servers = []
    total = mismatches = 0
    active = case["peak"]
    capabilities = copy.deepcopy(CAPABILITIES)
    capabilities["workspace"]["didChangeWatchedFiles"] = {"dynamicRegistration": True}
    command = [DART, "language-server", "--protocol=lsp", "--client-id=oracle"]
    if DART_CACHE:
        command.append("--cache=" + DART_CACHE)

    def folder(i):
        return {"uri": projects[i].as_uri(), "name": projects[i].name}

    def probe_context(i, analyzed):
        deadline = time.monotonic() + 30
        params = {"textDocument": {"uri": (projects[i] / "lib/entry.dart").as_uri()}}
        while True:
            probes = [server.request("textDocument/documentSymbol", params)[0] for server in servers]
            result = probes[0]
            if (isinstance(result, list) if analyzed else
                    isinstance(result, dict) and result.get("error", {}).get("code") == -32007):
                return
            if time.monotonic() >= deadline:
                raise RuntimeError(f"context {i} did not reach analyzed={analyzed}: {result}")
            time.sleep(0.1)

    def changed_roots(ids):
        nonlocal active
        previous = servers[0].analysis_completions
        removed = [i for i in active if i not in ids]
        added = [i for i in ids if i not in active]
        params = {"event": {"removed": [folder(i) for i in removed], "added": [folder(i) for i in added]}}
        for server in servers:
            server.notify("workspace/didChangeWorkspaceFolders", params)
        active = ids
        # With any driver left, Dart can resolve files outside its roots.
        # FileNotAnalyzed is a reliable barrier only after the last removal.
        if removed and not ids:
            probe_context(removed[0], False)
        if ids:
            probe_context(ids[0], True)
            if not servers[0].wait_analysis(previous, limit=90, quiet=1.0):
                raise RuntimeError("analysis after changing workspace folders did not finish")

    def rebuilt(phase):
        options = projects[active[0]] / "analysis_options.yaml"
        previous = servers[0].analysis_completions
        _write(options, f"# Driver capacity scenario: {phase}\n")
        for server in servers:
            server.notify("workspace/didChangeWatchedFiles", {"changes": [{"uri": options.as_uri(), "type": 2}]})
        if not servers[0].wait_analysis(previous, limit=90, quiet=1.5):
            raise RuntimeError("analysis after rebuilding contexts did not finish")

    def record(phase, expected_order):
        nonlocal total, mismatches
        for query in ("CapacityEntry", "CapacityDependency", f"CapacityDependency{case['survivors'][0]:02}"):
            row = {"project": "symbol_capacity", "phase": phase, "method": "workspace/symbol",
                   "pos": query, "activeProjects": active, "referenceCase": case,
                   "queryAfterReduction": query_after_reduction}
            for server in servers:
                row[server.name], _ = server.request("workspace/symbol", {"query": query})
                if not isinstance(row[server.name], list):
                    raise RuntimeError(f"{phase} {query} {server.name}: {row[server.name]}")
            same = row["dart"] == row["dartforge"]
            output.write(json.dumps(row, ensure_ascii=False) + "\n")
            output.flush()
            total += 1
            mismatches += not same
            print(f"{phase}: {query}: {'equal' if same else 'DIFFERENT'} "
                  f"({len(row['dart'])}/{len(row['dartforge'])})", flush=True)
            if query == "CapacityEntry":
                assert len(row["dart"]) == len(active), row["dart"]
                creation = (case["contextOrder"] if active == case["peak"] else
                            case["survivorContextOrder"] if active else [])
                expected = [f"CapacityEntry{i:02}" for i in creation]
                assert [s["name"] for s in row["dart"]] == expected, (phase, expected, row["dart"])
            if query == "CapacityDependency":
                expected = [f"CapacityDependency{i:02}" for i in expected_order]
                assert [s["name"] for s in row["dart"]] == expected, (phase, expected, row["dart"])

    try:
        servers = [Lsp("dart", command), Lsp("dartforge", [binary, "--stdio"],
                    env=dict(os.environ, DARTFORGE_SDK_LIB=SDK_LIB))]
        previous = servers[0].analysis_completions
        for server in servers:
            response, _ = server.request("initialize", {"processId": os.getpid(),
                "rootUri": None,
                "workspaceFolders": [folder(i) for i in active], "capabilities": capabilities,
                "initializationOptions": {"onlyAnalyzeProjectsWithOpenFiles": False}})
            if not isinstance(response, dict) or "capabilities" not in response:
                raise RuntimeError(f"initialize failed: {server.name}: {response}")
            server.notify("initialized", {})
        if not servers[0].wait_analysis(previous, limit=90, quiet=1.5):
            raise RuntimeError("initial analysis did not finish")
        if not query_after_reduction:
            record("expanded", case["initialOrder"])
        rebuilt("expanded")
        if not query_after_reduction:
            record("rebuilt_expanded", case["rebuiltOrder"])
        changed_roots(case["survivors"])
        record("reduced", case["retainedPairOrder"])
        changed_roots([])
        record("empty", [])
        changed_roots(case["survivors"])
        record("reopened", case["retainedPairOrder"])
        rebuilt("small")
        record("rebuilt_small", case["retainedPairOrder"])
    finally:
        for server in servers:
            server.close()
    print(f"complete ordered responses: {total - mismatches}/{total}", flush=True)
    return bool(mismatches)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output")
    parser.add_argument("binary")
    parser.add_argument("--query-after-reduction", action="store_true")
    parser.add_argument("--fixture", type=Path,
                        help="keep and reuse an oracle-owned fixture at stable absolute paths")
    args = parser.parse_args()
    if args.fixture:
        fixture = args.fixture.resolve()
        _prepare_fixture(fixture)
        with open(args.output, "w", encoding="utf-8") as output:
            return _run(output, os.path.abspath(args.binary), fixture, args.query_after_reduction)
    with tempfile.TemporaryDirectory(prefix="dartforge-symbol-capacity-") as temporary:
        _prepare_fixture(Path(temporary))
        with open(args.output, "w", encoding="utf-8") as output:
            return _run(output, os.path.abspath(args.binary), Path(temporary).resolve(), args.query_after_reduction)


if __name__ == "__main__":
    raise SystemExit(main())

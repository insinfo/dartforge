"""Compare complete workspace symbols through document and filesystem changes.

Usage: ORACLE_DART=... ORACLE_SDK_LIB=... python3 workspace_symbol_lifecycle.py
       <output.jsonl> <dartforge-lsp>

The temporary project exceeds the 500-symbol limit. An anchor document keeps
it analyzed while another document closes and files change; closing the anchor
then checks onlyAnalyzeProjectsWithOpenFiles. Exit status is 1 on any mismatch.
This checks correctness, not performance.
--create-before-first-query inserts the first new file after initial analysis,
before any workspace/symbol request has populated the index.
"""

import argparse
import copy
import json
import os
from pathlib import Path
import tempfile
import time

from oracle import CAPABILITIES, DART, SDK_LIB, Lsp


def _document_symbols(server, path):
    result, _ = server.request(
        "textDocument/documentSymbol", {"textDocument": {"uri": path.as_uri()}}
    )
    return result


def _wait_for_context(server, path, analyzed):
    # didOpen/didClose can change analysis roots asynchronously in Dart.
    # Retry the transient FileNotAnalyzed response rather than interpreting
    # an empty workspace during root creation as a parity result.
    deadline = time.monotonic() + 30
    while True:
        result = _document_symbols(server, path)
        if isinstance(result, list):
            if analyzed:
                return
        else:
            assert result.get("error", {}).get("code") == -32007, result
            if not analyzed:
                return
        if time.monotonic() >= deadline:
            raise RuntimeError(f"analysis context did not reach analyzed={analyzed}: {path}")
        time.sleep(0.1)


def _wait_for_analysis(server, previous):
    # Watched-file handlers finish before the analyzer has applied their work.
    # Wait for a new completion and a quiet interval, including debounce waves.
    if not server.wait_analysis(previous, limit=60):
        raise RuntimeError("Dart did not finish the changed project's analysis")


def _run(output, binary, root, create_before_first_query=False):
    lib = root / "lib"
    lib.mkdir()
    (root / ".dart_tool").mkdir()
    (root / "pubspec.yaml").write_text(
        'name: symbol_lifecycle\nenvironment:\n  sdk: ">=3.6.2 <4.0.0"\n',
        encoding="utf-8",
    )
    (root / ".dart_tool/package_config.json").write_text(
        json.dumps({"configVersion": 2, "packages": [{
            "name": "symbol_lifecycle", "rootUri": "../",
            "packageUri": "lib/", "languageVersion": "3.6",
        }]}), encoding="utf-8",
    )
    a, b, c, z, anchor = [lib / f"{name}.dart" for name in ("a", "b", "c", "z", "anchor")]
    original = "".join(f"class LifecycleA{i:03} {{}}\n" for i in range(310))
    a.write_text(original, encoding="utf-8")
    b.write_text("".join(f"class LifecycleB{i:03} {{}}\n" for i in range(310)), encoding="utf-8")
    anchor.write_text("void lifecycleAnchor() {}\n", encoding="utf-8")
    capabilities = copy.deepcopy(CAPABILITIES)
    capabilities["workspace"]["didChangeWatchedFiles"] = {"dynamicRegistration": True}
    servers = []
    mismatches = 0
    total = 0
    pending_completion = None

    def notify(method, params):
        nonlocal pending_completion
        if pending_completion is None:
            with servers[0].cv:
                pending_completion = servers[0].analysis_completions
        for server in servers:
            server.notify(method, params)

    def opened(path, text):
        notify("textDocument/didOpen", {"textDocument": {
            "uri": path.as_uri(), "languageId": "dart", "version": 1, "text": text,
        }})

    def watched(path, kind):
        notify("workspace/didChangeWatchedFiles", {"changes": [{
            "uri": path.as_uri(), "type": kind,
        }]})

    def record(phase, analyzed=True):
        nonlocal mismatches, total, pending_completion
        _wait_for_context(servers[0], a, analyzed)
        if analyzed:
            _wait_for_analysis(servers[0], pending_completion)
            _wait_for_context(servers[1], a, True)
        pending_completion = None
        for query in ("Lifecycle", "LifecycleA000", "LifecycleUnsaved",
                      "LifecycleCreated", "LifecycleDisk", "a", "e"):
            row = {"project": "symbol_lifecycle", "phase": phase,
                   "method": "workspace/symbol", "pos": query}
            for server in servers:
                row[server.name], _ = server.request("workspace/symbol", {"query": query})
                if not isinstance(row[server.name], list):
                    raise RuntimeError(f"{phase} {query} {server.name}: {row[server.name]}")
            if phase in ("initial", "created_before_first_query") and query == "Lifecycle":
                assert len(row["dart"]) == 500, "the fixture must exercise the result limit"
            same = row["dart"] == row["dartforge"]
            total += 1
            mismatches += not same
            output.write(json.dumps(row, ensure_ascii=False) + "\n")
            output.flush()
            print(f"{phase}: {query}: Dart={len(row['dart'])}, "
                  f"DartForge={len(row['dartforge'])}, equal={same}", flush=True)

    try:
        servers.append(Lsp("dart", [DART, "language-server", "--protocol=lsp",
                                    "--client-id=oracle-lifecycle"]))
        servers.append(Lsp("dartforge", [str(binary), "--stdio"],
                           env=dict(os.environ, DARTFORGE_SDK_LIB=SDK_LIB)))
        for server in servers:
            result, _ = server.request("initialize", {
                "processId": os.getpid(), "rootUri": root.as_uri(),
                "workspaceFolders": [{"uri": root.as_uri(), "name": root.name}],
                "capabilities": capabilities,
                "initializationOptions": {"onlyAnalyzeProjectsWithOpenFiles": True},
            })
            assert isinstance(result, dict) and "error" not in result, result
            server.notify("initialized", {})
        opened(anchor, anchor.read_text(encoding="utf-8"))
        _wait_for_context(servers[0], anchor, True)
        opened(a, original)
        if create_before_first_query:
            _wait_for_context(servers[0], a, True)
            _wait_for_analysis(servers[0], pending_completion)
            pending_completion = None
            c.write_text("class LifecycleCreated {}\n", encoding="utf-8")
            watched(c, 1)
            record("created_before_first_query")
        else:
            record("initial")
        notify("textDocument/didChange", {
            "textDocument": {"uri": a.as_uri(), "version": 2},
            "contentChanges": [{"text": "class LifecycleUnsaved {}\n" +
                                original.replace("LifecycleA000", "LifecycleRenamed")}],
        })
        record("unsaved")
        notify("textDocument/didClose", {"textDocument": {"uri": a.as_uri()}})
        record("closed")
        c.write_text("class LifecycleCreated {}\n", encoding="utf-8")
        watched(c, 1)
        record("created")
        a.write_text("class LifecycleDisk {}\n" + original, encoding="utf-8")
        watched(a, 2)
        record("disk_changed")
        b.unlink()
        watched(b, 3)
        record("deleted")
        c.rename(z)
        watched(c, 3)
        watched(z, 1)
        record("renamed")
        notify("textDocument/didClose", {"textDocument": {"uri": anchor.as_uri()}})
        record("last_closed", analyzed=False)
    finally:
        for server in servers:
            server.close()
    print(f"Complete ordered responses: {total - mismatches}/{total}", flush=True)
    return bool(mismatches)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--create-before-first-query", action="store_true")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="dartforge-symbol-lifecycle-") as directory:
        with args.output.open("w", encoding="utf-8") as output:
            return _run(output, args.binary.resolve(), Path(directory).resolve(),
                        args.create_before_first_query)


if __name__ == "__main__":
    raise SystemExit(main())

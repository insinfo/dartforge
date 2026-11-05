"""Audit analysis readiness without changing or normalizing symbol responses.

Usage: ORACLE_DART=... ORACLE_SDK_LIB=... python3 workspace_symbol_readiness.py
       <output.jsonl> <dartforge-lsp> <project> [--mode legacy|cycle|resolved|settled]

Open the same first Dart file as ORACLE_OPEN_FILES=1 in oracle.py. Legacy mode
uses its status wait and one-second delay. Cycle mode also requires a new busy
status. Resolved mode adds identical documentSymbol readiness requests to both
servers. These are distinct client sequences, not interchangeable parity runs.
Settled mode resolves the anchor, then observes a shared idle interval in both.
The sidecar .trace.json contains protocol metadata and status counts, not source
text or full response bodies. Exit status is 1 on any ordered JSON difference.
"""

import argparse
import json
import os
from pathlib import Path
import time

from oracle import CAPABILITIES, DART, DART_CACHE, SDK_LIB, Lsp, analysis_states, settle_initial_analysis


class Trace:
    def __init__(self):
        self.started = time.perf_counter()
        self.events = []
        self.requests = {}

    def observe(self, server):
        def record(direction, message):
            method = message.get("method")
            request_id = message.get("id")
            if direction == "sent" and method and request_id is not None:
                self.requests[(server, request_id)] = method
            event = {"server": server, "direction": direction,
                     "ms": (time.perf_counter() - self.started) * 1000,
                     "method": method, "id": request_id}
            if method == "$/analyzerStatus":
                event["isAnalyzing"] = message["params"]["isAnalyzing"]
            if method in ("textDocument/didOpen", "textDocument/didChange"):
                document = message["params"]["textDocument"]
                event["uri"] = document["uri"]
                event["version"] = document["version"]
            if direction == "received" and not method and request_id is not None:
                event["requestMethod"] = self.requests.get((server, request_id))
                if "error" in message:
                    event["error"] = message["error"]
                else:
                    result = message.get("result")
                    event["resultType"] = type(result).__name__
                    if isinstance(result, list):
                        event["resultLength"] = len(result)
            self.events.append(event)
        return record

    def mark(self, phase, servers):
        states = analysis_states(servers)
        self.events.append({"phase": phase, "ms": (time.perf_counter() - self.started) * 1000,
                            "states": states})


def _wait_cycle(server, starts, completions, limit=90):
    deadline = time.monotonic() + limit
    with server.cv:
        while not (server.analysis_starts > starts and server.analysis_completions > completions
                   and server.analyzing is False):
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                return False
            server.cv.wait(timeout=remaining)
    return server.wait_analysis(completions, limit=max(0, deadline - time.monotonic()))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("binary")
    parser.add_argument("project", type=Path)
    parser.add_argument("--mode", choices=("legacy", "cycle", "resolved", "settled"), default="legacy")
    args = parser.parse_args()
    root = args.project.resolve()
    files = sorted(p for directory in ("lib", "bin") for p in (root / directory).rglob("*.dart"))
    if not files:
        raise RuntimeError(f"project has no Dart files: {root}")
    anchor = files[0]
    command = [DART, "language-server", "--protocol=lsp", "--client-id=oracle"]
    if DART_CACHE:
        command.append("--cache=" + DART_CACHE)
    trace = Trace()
    servers = []
    mismatches = 0
    try:
        servers = [Lsp("dart", command, observer=trace.observe("dart")),
                   Lsp("dartforge", [os.path.abspath(args.binary), "--stdio"],
                       env=dict(os.environ, DARTFORGE_SDK_LIB=SDK_LIB), observer=trace.observe("dartforge"))]
        with args.output.open("w", encoding="utf-8") as output:
            for server in servers:
                response, _ = server.request("initialize", {"processId": os.getpid(), "rootUri": root.as_uri(),
                    "workspaceFolders": [{"uri": root.as_uri(), "name": root.name}], "capabilities": CAPABILITIES,
                    "initializationOptions": {"onlyAnalyzeProjectsWithOpenFiles": True}})
                if not isinstance(response, dict) or "capabilities" not in response:
                    raise RuntimeError(f"initialize failed: {server.name}: {response}")
                server.notify("initialized", {})
            trace.mark("before_open", servers)
            with servers[0].cv:
                starts = servers[0].analysis_starts
                completions = servers[0].analysis_completions
            source = anchor.read_text(encoding="utf-8")
            for server in servers:
                server.notify("textDocument/didOpen", {"textDocument": {
                    "uri": anchor.as_uri(), "languageId": "dart", "version": 1, "text": source}})
            trace.mark("after_open", servers)
            if args.mode in ("legacy", "settled"):
                ready = servers[0].wait_analysis(completions)
            else:
                ready = _wait_cycle(servers[0], starts, completions)
            if not ready:
                raise RuntimeError(f"{args.mode} analysis barrier timed out")
            trace.mark("status_ready", servers)
            time.sleep(1.0)
            if args.mode == "settled":
                readiness = settle_initial_analysis(servers, [anchor])
                output.write(json.dumps({"project": root.name, "method": "oracle/initialReadiness",
                                         "mode": args.mode, **readiness}) + "\n")
                output.flush()
                trace.mark("both_idle", servers)
            if args.mode == "resolved":
                deadline = time.monotonic() + 90
                params = {"textDocument": {"uri": anchor.as_uri()}}
                while True:
                    probes = []
                    for server in servers:
                        remaining = deadline - time.monotonic()
                        if remaining <= 0:
                            raise RuntimeError("anchor resolution timed out")
                        probe, _ = server.request("textDocument/documentSymbol", params, limit=remaining)
                        if probe is not None and not isinstance(probe, list):
                            error = probe.get("error") if isinstance(probe, dict) else None
                            if not isinstance(error, dict) or error.get("code") != -32007:
                                raise RuntimeError(f"{server.name}: anchor resolution failed: {probe}")
                        probes.append(probe)
                    if all(probe is None or isinstance(probe, list) for probe in probes):
                        break
                    time.sleep(min(0.1, max(0, deadline - time.monotonic())))
                with servers[0].cv:
                    previous = servers[0].analysis_completions
                    busy = servers[0].analyzing
                if busy and not servers[0].wait_analysis(previous, limit=max(0, deadline - time.monotonic())):
                    raise RuntimeError("analysis after the readiness request did not finish")
                trace.mark("resolved_ready", servers)
            trace.mark("before_queries", servers)
            for query in ("Arg", "parse", "Scanner", "ctx", "Ponto", "join", "a", "e"):
                row = {"project": root.name, "method": "workspace/symbol", "pos": query,
                       "readinessMode": args.mode, "rootUri": root.as_uri(), "anchor": anchor.as_uri(),
                       "referenceCache": DART_CACHE, "sdkLib": SDK_LIB}
                for server in servers:
                    row[server.name], _ = server.request("workspace/symbol", {"query": query})
                    if not isinstance(row[server.name], list):
                        raise RuntimeError(f"{query} {server.name}: {row[server.name]}")
                same = row["dart"] == row["dartforge"]
                mismatches += not same
                output.write(json.dumps(row, ensure_ascii=False) + "\n")
                output.flush()
                print(f"{args.mode} {query}: {'equal' if same else 'DIFFERENT'} "
                      f"({len(row['dart'])}/{len(row['dartforge'])})", flush=True)
            trace.mark("after_queries", servers)
    finally:
        for server in servers:
            server.close()
        args.output.with_suffix(".trace.json").write_text(
            json.dumps(trace.events, ensure_ascii=False), encoding="utf-8")
    print(f"complete ordered responses: {8 - mismatches}/8", flush=True)
    return bool(mismatches)


if __name__ == "__main__":
    raise SystemExit(main())

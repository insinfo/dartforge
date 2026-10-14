"""Build and compare guarded ARC production binaries from one fixed compiler."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import statistics
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument("phase", choices=["build", "measure"])
parser.add_argument("--compiler", type=Path, required=True)
args = parser.parse_args()
root = Path.cwd()
out = root / "bench/results/2026-10-14-native-arc-tag-guard-performance"
work = root / "target/native-provenance/arc-tag-guard-performance"
out.mkdir(parents=True, exist_ok=True)
work.mkdir(parents=True, exist_ok=True)
sdk = Path("/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib")
sources = {"json": root / "bench/comparison/json_encode.dart",
           "objects": root / "bench/performance/objects_in_collections.dart"}

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

env = {k: v for k, v in os.environ.items() if not k.startswith("DARTFORGE_")}
env.update(DARTFORGE_CLANG="/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang",
           DARTFORGE_ARC_PREPARED="1", DARTFORGE_ARC_CHECK="0",
           DARTFORGE_ARC_PORT_PRUNING="1", DARTFORGE_FACTORY_RECEIVER_PROVENANCE="1")
ledger_path = out / "validation.json"

def save():
    ledger_path.write_text(json.dumps(ledger, indent=2) + "\n")

def execute(label, command, execution):
    start = time.perf_counter_ns()
    run = subprocess.run([str(v) for v in command], cwd=root, env=execution,
                         capture_output=True, timeout=1800)
    wall = time.perf_counter_ns() - start
    (out / (label + ".stdout")).write_bytes(run.stdout)
    (out / (label + ".stderr")).write_bytes(run.stderr)
    ledger["runs"].append({"label": label, "command": [str(v) for v in command],
                           "environment": {k: v for k, v in execution.items() if k.startswith("DARTFORGE_")},
                           "exit": run.returncode, "wall_ns": wall,
                           "stdout_sha256": hashlib.sha256(run.stdout).hexdigest(),
                           "stderr_sha256": hashlib.sha256(run.stderr).hexdigest()})
    save()
    assert run.returncode == 0 and not run.stderr, label
    return run.stdout

def content(kind, output):
    if kind == "json":
        a = json.loads(output)
        return [(s["scenario"], s["json"]["encoded_units"], s["json"]["checksum"],
                 s["string_buffer"]["encoded_units"], s["string_buffer"]["checksum"])
                for s in a["scenarios"]]
    rows = []
    for line in output.decode().splitlines():
        m = re.fullmatch(r"([^:]+): ([0-9 ]+) us \| (.+)", line)
        assert m, line
        rows.append((m[1], m[3]))
    return rows

if args.phase == "build":
    assert not ledger_path.exists(), "Keep completed or partial evidence; choose a new directory to rerun"
    inputs = [*sources.values(), root / "bench/performance/common.dart", Path(__file__)]
    ledger = {"compiler": str(args.compiler.resolve()), "compiler_sha256": sha(args.compiler),
              "inputs": {str(p.resolve()): sha(p) for p in inputs}, "runs": [],
              "artifacts": {}, "oracle": {}, "build_complete": False,
              "scope": "Prepared ARC, same compiler and source, tag guard off/on; production effects instrumentation absent. JSON correctness additionally runs production binaries with ARC_CHECK=1 and GC stress. Full-sized object kernels validate VM results with production checks disabled; separate small ordinary fixture covers diagnostic/stress ownership."}
    save()
    for kind, source in sources.items():
        extra = ["--validate"] if kind == "json" else []
        ledger["oracle"][kind] = content(kind, execute(kind + "-vm", [sdk.parent / "bin/dart", source, *extra], env))
        for setting in ["off", "on"]:
            execution = dict(env, DARTFORGE_NO_ARC_TAG_GUARDS="1" if setting == "off" else "0")
            folder = work / kind / setting
            folder.mkdir(parents=True, exist_ok=True)
            binary = folder / "program"
            execute(kind + "-" + setting + "-build", [args.compiler, "compile-native", source,
                    "--sdk", sdk, "--optimize", "--memory=arc", "--exceptions=check",
                    "--roots=shadow", "--stack-trace=none", "-o", binary], execution)
            ledger["artifacts"][kind + "-" + setting] = {"path": str(binary), "sha256": sha(binary), "bytes": binary.stat().st_size}
            for stress in ([False, True] if kind == "json" else [False]):
                check_env = dict(execution, DARTFORGE_ARC_CHECK="1" if kind == "json" else "0")
                if stress:
                    check_env["DARTFORGE_GC_STRESS"] = "1"
                output = execute(kind + "-" + setting + ("-stress" if stress else "-normal"), [binary, *extra], check_env)
                assert content(kind, output) == [tuple(v) for v in ledger["oracle"][kind]]
            save()
    ledger["build_complete"] = True
    save()
else:
    ledger = json.loads(ledger_path.read_text())
    assert ledger["build_complete"] and sha(args.compiler) == ledger["compiler_sha256"]
    assert all(sha(p) == h for p, h in ledger["inputs"].items())
    assert all(sha(a["path"]) == a["sha256"] for a in ledger["artifacts"].values())
    records = []
    for pair, order in enumerate([["off", "on"], ["on", "off"], ["off", "on"]], 1):
        for kind in sources:
            for setting in order:
                a = ledger["artifacts"][kind + "-" + setting]
                output = execute(f"{kind}-pair-{pair}-{setting}", [a["path"]], env)
                assert content(kind, output) == [tuple(v) for v in ledger["oracle"][kind]]
                if kind == "json":
                    rows = json.loads(output)["scenarios"]
                    kernels = {s["scenario"] + "/" + k: s[k]["elapsed_micros"] / s[k]["iterations"]
                               for s in rows for k in ["json", "string_buffer"]}
                else:
                    kernels = {m[1]: statistics.median([int(v) for v in m[2].split()][1:])
                               for m in [re.fullmatch(r"([^:]+): ([0-9 ]+) us \| (.+)", line)
                                         for line in output.decode().splitlines()]}
                records.append({"pair": pair, "kind": kind, "setting": setting, "kernels_us": kernels})
    summary = {}
    for kind in sources:
        names = next(r["kernels_us"] for r in records if r["kind"] == kind)
        for name in names:
            values = {s: statistics.median(r["kernels_us"][name] for r in records if r["kind"] == kind and r["setting"] == s) for s in ["off", "on"]}
            summary[kind + "/" + name] = dict(values, on_over_off=values["on"] / values["off"])
    (out / "measurement.json").write_text(json.dumps({"records": records, "summary": summary,
        "scope": "Three alternating pairs in a root-coordinated quiet window; JSON microseconds per encoding and object kernels median excluding first sample. Production ARC checks disabled; no upstream or global ARC speed claim."}, indent=2) + "\n")
    print(json.dumps(summary, indent=2))

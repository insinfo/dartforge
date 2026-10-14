"""Compare native compilation with warm SDK caches and exact VM output checks."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import time


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("before", type=Path)
    parser.add_argument("after", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--sdk", type=Path, required=True)
    parser.add_argument("--calls", type=int, default=500)
    parser.add_argument("--repetitions", type=int, default=3)
    args = parser.parse_args()
    if args.repetitions < 1:
        parser.error("--repetitions must be positive")
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=True)
    source = args.output / "main.dart"
    subprocess.run([sys.executable, str(Path(__file__).with_name("generate_pending_calls.py")),
                    str(source), "--calls", str(args.calls)], check=True)
    compilers = {"before": args.before.resolve(), "after": args.after.resolve()}
    manifest = {"complete": False, "scope": "Warm-SDK whole-compilation wall time for this generated workload only; not executable runtime or general compiler parity.",
                "calls": args.calls, "repetitions": args.repetitions,
                "source_sha256": sha(source),
                "compiler_sha256": {label: sha(path) for label, path in compilers.items()},
                "runs": [], "pooling_disabled_for_both": True,
                "object_cache_disabled_for_both": True}
    env = os.environ.copy()
    for key in ("DARTFORGE_MEMORY", "DARTFORGE_GC_STRESS", "DARTFORGE_NO_TABLE_PRUNING"):
        env.pop(key, None)
    env.update({"DARTFORGE_ARC_PREPARED": "1", "DARTFORGE_ARC_CHECK": "1",
                "DARTFORGE_EFFECTS": "check", "DARTFORGE_AOT_CONST_ARRAY_POOLING": "0",
                "DARTFORGE_CACHE_OBJ": "0"})

    def save():
        (args.output / "measurement.json").write_text(json.dumps(manifest, indent=2) + "\n")

    def execute(label, command, execution_env, timed=False):
        started = time.perf_counter()
        result = subprocess.run([str(value) for value in command], env=execution_env, capture_output=True, timeout=1200)
        seconds = time.perf_counter() - started
        (args.output / f"{label}.stdout").write_bytes(result.stdout)
        (args.output / f"{label}.stderr").write_bytes(result.stderr)
        row = {"label": label, "command": [str(value) for value in command],
               "environment": {key: value for key, value in execution_env.items() if key.startswith("DARTFORGE_")},
               "exit_code": result.returncode, "timing_sample": timed,
               "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(),
               "stderr_sha256": hashlib.sha256(result.stderr).hexdigest()}
        if timed:
            row["seconds"] = seconds
        manifest["runs"].append(row)
        save()
        if result.returncode:
            raise RuntimeError(f"{label} failed; inspect saved stderr")
        return result, row

    oracle, _ = execute("vm", [args.sdk.parent / "bin/dart", source], env)
    if oracle.stderr:
        raise RuntimeError("VM oracle wrote stderr")
    for phase in range(args.repetitions + 1):
        order = ("before", "after") if phase % 2 == 0 else ("after", "before")
        for label in order:
            prefix = f"warmup-{label}" if phase == 0 else f"sample-{phase}-{label}"
            binary = args.output / f"{prefix}.program"
            command = [compilers[label], "compile-native", source, "--sdk", args.sdk,
                       "--optimize", "--memory=arc", "--exceptions=check", "--roots=shadow",
                       "--stack-trace=none", "-o", binary]
            _, row = execute(prefix + "-build", command, env, timed=phase > 0)
            row.update({"compiler": label, "binary_bytes": binary.stat().st_size, "binary_sha256": sha(binary)})
            execution, run = execute(prefix + "-run", [binary], env)
            run["exact_vm"] = execution.stdout == oracle.stdout and not execution.stderr
            if not run["exact_vm"]:
                save()
                raise RuntimeError(f"{prefix} differs from the VM")
            if phase == 0:
                stress = dict(env, DARTFORGE_GC_STRESS="1")
                execution, run = execute(prefix + "-stress", [binary], stress)
                run["exact_vm"] = execution.stdout == oracle.stdout and not execution.stderr
                if not run["exact_vm"]:
                    save()
                    raise RuntimeError(f"{prefix} under GC stress differs from the VM")
            save()
            print(prefix + ": exact VM", flush=True)
    manifest["median_compile_seconds"] = {
        label: statistics.median(row["seconds"] for row in manifest["runs"]
                                 if row["timing_sample"] and row["compiler"] == label)
        for label in compilers}
    manifest["inputs_unchanged"] = sha(source) == manifest["source_sha256"] and all(
        sha(path) == manifest["compiler_sha256"][label] for label, path in compilers.items())
    manifest["complete"] = True
    save()
    if not manifest["inputs_unchanged"]:
        raise RuntimeError("Compiler or source changed during measurement")


if __name__ == "__main__":
    main()

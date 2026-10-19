"""Exercise the oracle-only CLI from the repository root on macOS."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["before", "after", "sdk", "node-dir", "out"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env["DARTFORGE_DART_SDK"] = str(args.sdk.resolve())
    env["PATH"] = os.pathsep.join([
        str(args.node_dir.resolve()), str(args.sdk.resolve() / "bin"), env["PATH"],
    ])
    rows = []
    cases = [
        ("before-web", args.before, [], 1, False),
        ("after-web", args.after, [], 0, False),
        ("after-native", args.after, ["--native"], 1, False),
        ("after-missing-node", args.after, [], 1, True),
    ]
    with tempfile.TemporaryDirectory(prefix="oracle no node ") as missing:
        for name, binary, extra, expected, missing_node in cases:
            child_env = env.copy()
            if missing_node:
                child_env["PATH"] = missing
            command = [
                str(binary.resolve()), "--no-forge", "--filter", "209_js_interop",
                "--no-cache", "--jobs", "2", *extra,
            ]
            result = subprocess.run(command, env=child_env, capture_output=True, timeout=180)
            (args.out / (name + ".stdout")).write_bytes(result.stdout)
            (args.out / (name + ".stderr")).write_bytes(result.stderr)
            rows.append({
                "name": name, "command": command, "expected_exit": expected,
                "exit": result.returncode, "missing_node": missing_node,
                "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(),
                "stderr_sha256": hashlib.sha256(result.stderr).hexdigest(),
            })
            (args.out / "results.json").write_text(json.dumps(rows, indent=2) + "\n")
            assert result.returncode == expected, (name, result.stdout, result.stderr)
            assert b"209_js_interop" in result.stdout, name
            if missing_node:
                assert b"ORACLE!" in result.stdout and b"node" in result.stdout.lower()
            else:
                assert b"not available on this platform" in result.stdout, name
            print(name, result.returncode, flush=True)


if __name__ == "__main__":
    main()

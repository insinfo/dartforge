#!/usr/bin/env python3
"""Exercise one compiled build script against relocated Cargo source trees."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-script", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target", default="aarch64-apple-darwin")
    args = parser.parse_args()
    executable = args.build_script.resolve()
    records = []
    with tempfile.TemporaryDirectory(prefix="dartforge-relocated-build-") as temporary:
        for label, value in [("first", "one"), ("changed", "two"), ("relocated", "one")]:
            workspace = Path(temporary) / label
            for crate in ["emit_native", "runtime", "runtime_static"]:
                source = workspace / "crates" / crate / "src"
                source.mkdir(parents=True)
                (source / "probe.rs").write_text(f"// {crate}: {value}\n")
            manifest = workspace / "crates" / "emit_native"
            output = workspace / "output"
            output.mkdir()
            environment = os.environ.copy()
            environment.update(CARGO_MANIFEST_DIR=str(manifest), OUT_DIR=str(output),
                               TARGET=args.target, DARTFORGE_RUNTIME_NO_PRECOMPILE="1")
            result = subprocess.run([str(executable)], env=environment, capture_output=True, text=True)
            facts = {}
            for line in result.stdout.splitlines():
                if line.startswith("cargo::rustc-env="):
                    key, value = line.removeprefix("cargo::rustc-env=").split("=", 1)
                    facts[key] = value
            records.append(dict(label=label, exit=result.returncode, facts=facts,
                                stdout=result.stdout, stderr=result.stderr))
    args.output.write_text(json.dumps(dict(build_script=str(executable),
        build_script_sha256=hashlib.sha256(executable.read_bytes()).hexdigest(), records=records), indent=2) + "\n")
    assert all(record["exit"] == 0 for record in records), records
    for key in ["DARTFORGE_EMITTER_HASH", "DARTFORGE_RUNTIME_SOURCE_HASH",
                "DARTFORGE_RUNTIME_MAIN", "DARTFORGE_RUNTIME_DLL", "DARTFORGE_RUNTIME_PRODUCTION"]:
        first, changed, relocated = [record["facts"][key] for record in records]
        assert first != changed, f"{key} ignored changed sources at Cargo's current manifest directory"
        assert first == relocated, f"{key} depends on the checkout's absolute path"
    print("One reused build script follows current sources and preserves relocation-independent fingerprints")


if __name__ == "__main__":
    main()

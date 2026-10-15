#!/usr/bin/env python3
"""Freeze and compile the probe against the actual current release artifacts."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import shlex
import subprocess

root = Path(__file__).resolve().parents[3]
os.chdir(root)
stamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
archive = Path(__file__).resolve().parent / "producer" / stamp
archive.mkdir(parents=True)
output = root / "target" / "jit-eager-stubs-producer" / stamp / "probe"
output.parent.mkdir(parents=True)
deps = root / "target/jit-readiness-build/release/deps"
source = Path(__file__).with_name("probe.rs")
selected = {}
for name in ["dartforge_jit", "dartforge_emit_native", "serde_json"]:
    paths = list(deps.glob(f"lib{name}-*.rlib"))
    if not paths:
        raise SystemExit(f"No current artifact for {name}")
    selected[name] = max(paths, key=lambda path: path.stat().st_mtime_ns)
hash_file = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
inputs = [source, *selected.values(), root / "crates/jit/src/reload.rs", root / "crates/jit/src/ffi.rs"]
command = ["rustc", str(source), "--edition=2024", "-O", "-o", str(output),
    "-L", f"dependency={deps}", "-C", f"linker={os.environ['DARTFORGE_CLANG']}"]
# Emission and linking use independent executables: Cargo dependency feature
# unification can produce different runtime ABIs in the two existing rlib graphs.
# Loading both no_mangle runtimes into one executable is invalid.
for name in ["dartforge_jit", "serde_json"]:
    command += ["--extern", f"{name}={selected[name]}"]
for path in [Path(os.environ["DARTFORGE_LLVM_DIR"]) / "lib", root / "target/jit-arc-readiness/zstd"]:
    command += ["-L", f"native={path}"]
for path in (root / "target/jit-readiness-build/release/build").glob("llvm-sys-*/out"):
    if (path / "libtargetwrappers.a").exists():
        command += ["-L", f"native={path}"]
command += shlex.split(os.environ.get("RUSTFLAGS", ""))
record = {"status": "building", "command": command,
    "input_sha256": {str(path): hash_file(path) for path in inputs},
    "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
    "scope": "Standalone probe; identical saved IR emission separated from JIT linking. No application execution."}
manifest = archive / "validation.json"
manifest.write_text(json.dumps(record, indent=2) + "\n")
print(archive, flush=True)
emit_output = output.with_name("emit-probe")
emit_command = command.copy()
emit_command[emit_command.index(str(output))] = str(emit_output)
for index, arg in enumerate(emit_command):
    if arg.startswith("dartforge_jit="):
        emit_command[index] = f"dartforge_emit_native={selected['dartforge_emit_native']}"
emit_command += ["--cfg", "emit_only"]
record["emit_command"] = emit_command
manifest.write_text(json.dumps(record, indent=2) + "\n")
with (archive / "stdout").open("w") as out, (archive / "stderr").open("w") as err:
    result = subprocess.run(command, stdout=out, stderr=err)
if result.returncode == 0:
    with (archive / "emit.stdout").open("w") as out, (archive / "emit.stderr").open("w") as err:
        result = subprocess.run(emit_command, stdout=out, stderr=err)
record["exit"] = result.returncode
record["inputs_unchanged"] = all(hash_file(path) == record["input_sha256"][str(path)] for path in inputs)
record["status"] = "passed" if result.returncode == 0 and record["inputs_unchanged"] else "failed"
if result.returncode == 0:
    record["probe"] = str(output)
    record["probe_sha256"] = hash_file(output)
    record["emit_probe"] = str(emit_output)
    record["emit_probe_sha256"] = hash_file(emit_output)
manifest.write_text(json.dumps(record, indent=2) + "\n")
print(json.dumps(record, indent=2), flush=True)
if result.returncode:
    print((archive / "stderr").read_text()[-6000:])
raise SystemExit(0 if record["status"] == "passed" else 1)

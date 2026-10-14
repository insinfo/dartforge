"""Build and freeze a native CLI with a checked source manifest on the SSD."""
import argparse
import hashlib
import json
import os
import shutil
import subprocess
from pathlib import Path

root = Path.cwd()
parser = argparse.ArgumentParser()
parser.add_argument("--output-dir", type=Path)
parser.add_argument("--producer-dir", type=Path)
args = parser.parse_args()
out = (args.output_dir or root / "bench/results/2026-10-14-rea-native-investigation").resolve()
work = (args.producer_dir or root / "target/rea-pre-prune-producer").resolve()
assert not (work / "dartforge").exists(), "Refusing to overwrite a frozen producer; choose --producer-dir"
out.mkdir(parents=True, exist_ok=True)
work.mkdir(parents=True, exist_ok=True)
sha = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()


def sources():
    paths = [root / "Cargo.toml", root / "Cargo.lock"]
    for parent in [root / "crates", root / "native_sdk"]:
        paths.extend(path for path in parent.rglob("*") if path.is_file() and
                     (path.suffix in {".rs", ".dart", ".c", ".h", ".json"} or path.name == "Cargo.toml"))
    return {str(path.relative_to(root)): sha(path) for path in sorted(paths)}


before = sources()
head = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
(work / "source.json").write_text(json.dumps(before, indent=2) + "\n")
(work / "source.patch").write_bytes(subprocess.check_output(["git", "diff"]))
env = os.environ.copy()
env.update(CARGO_TARGET_DIR=str(root / "target/p4"), TMPDIR=str(root / "target/tmp-rea-build"),
           LLVM_SYS_221_PREFIX="/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64",
           DARTFORGE_CLANG="/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang",
           DARTFORGE_TEST_SDK_LIB="/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib")
Path(env["TMPDIR"]).mkdir(parents=True, exist_ok=True)
command = ["cargo", "build", "--locked", "-j2", "-p", "dartforge-cli", "--release", "--features", "native"]
with (out / "producer-build.log").open("wb") as log:
    result = subprocess.run(command, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=1800)
after = sources()
ledger = {"head": head, "command": command, "exit": result.returncode,
          "source_scope": "Cargo manifests and Rust/Dart/C/header/JSON sources under crates and native_sdk, including tests.",
          "source_hashes": before, "source_stable": before == after,
          "changed_during_build": [path for path in sorted(set(before) | set(after)) if before.get(path) != after.get(path)]}
if result.returncode == 0 and before == after:
    destination = work / "dartforge"
    shutil.copy2(Path(env["CARGO_TARGET_DIR"]) / "release/dartforge", destination)
    ledger.update(compiler=str(destination), compiler_sha256=sha(destination), compiler_bytes=destination.stat().st_size)
(work / "provenance.json").write_text(json.dumps(ledger, indent=2) + "\n")
(out / "producer-build.json").write_text(json.dumps({key: value for key, value in ledger.items() if key != "source_hashes"}, indent=2) + "\n")
assert result.returncode == 0 and before == after, ledger["changed_during_build"]

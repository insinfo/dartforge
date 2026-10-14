"""Launch the ordinary Explorer and retain its actual wait status for GUI QA."""
import argparse
import hashlib
import json
import os
import subprocess
from pathlib import Path

root = Path.cwd()
parser = argparse.ArgumentParser()
parser.add_argument("--profile", default="combined_default", choices=["off", "on", "combined_default"])
parser.add_argument("--work-dir", type=Path)
args = parser.parse_args()
work = (args.work_dir or root / "target/native-receiver-gui").resolve()
work.mkdir(parents=True, exist_ok=True)
comparison = json.loads((root / "bench/results/2026-10-14-native-canonical-cache/sizes/comparison.json").read_text())
artifact = comparison["profiles"][args.profile]
binary = Path(artifact["binary"])
assert hashlib.sha256(binary.read_bytes()).hexdigest() == artifact["binary_sha256"]
command = [str(binary), str(root / "target/file-explorer-fixture"), "--lang=en"]
env = {key: value for key, value in os.environ.items() if not key.startswith("DARTFORGE_")}
env["TMPDIR"] = str(work)
with (work / "stdout").open("wb") as stdout, (work / "stderr").open("wb") as stderr:
    child = subprocess.Popen(command, cwd="/Volumes/ssd_dock/MyProjectsDart/dart_ui", env=env,
                             stdout=stdout, stderr=stderr)
    launch = {"pid": child.pid, "command": command, "binary_sha256": artifact["binary_sha256"]}
    (work / "launch.json").write_text(json.dumps(launch, indent=2) + "\n")
    print(json.dumps(launch), flush=True)
    try:
        status = child.wait(timeout=600)
    except subprocess.TimeoutExpired:
        child.terminate()
        status = child.wait(timeout=15)
        raise
(work / "exit.json").write_text(json.dumps({"exit_code": status}) + "\n")
raise SystemExit(status)

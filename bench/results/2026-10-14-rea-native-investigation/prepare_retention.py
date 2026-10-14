"""Reproduce the frozen size build with causal pruning and LLVM IR evidence."""
import hashlib
import json
import os
import subprocess
from pathlib import Path

root = Path.cwd()
out = root / "bench/results/2026-10-14-rea-native-investigation"
work = root / "target/rea-native-investigation/retention"
work.mkdir(parents=True, exist_ok=True)
baseline = json.loads((root / "bench/results/2026-10-14-native-platform-specialization/sizes/comparison.json").read_text())
sha = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
assert sha(baseline["compiler"]) == baseline["compiler_sha256"]
command = list(baseline["profiles"]["on"]["command"])
command[-1] = str(work / "program")
env = {key: value for key, value in os.environ.items() if not key.startswith("DARTFORGE_")}
env.update(baseline["environment"])
env.update(TMPDIR=str(work), DARTFORGE_KEEP_IR="1", DARTFORGE_WHY=str(work / "retention.tsv"))
ledger = {"command": command, "compiler_sha256": baseline["compiler_sha256"],
          "baseline_binary_sha256": baseline["profiles"]["on"]["binary_sha256"],
          "environment": {key: value for key, value in env.items() if key.startswith("DARTFORGE_") or key == "TMPDIR"},
          "purpose": "Correctness and retention diagnosis only, no performance measurement.", "complete": False}
(out / "retention-build.json").write_text(json.dumps(ledger, indent=2) + "\n")
with (out / "retention-build.stdout").open("wb") as stdout, (out / "retention-build.stderr").open("wb") as stderr:
    result = subprocess.run(command, env=env, stdout=stdout, stderr=stderr, timeout=1800)
ledger["exit"] = result.returncode
if result.returncode == 0:
    ledger["binary_sha256"] = sha(work / "program")
    ledger["binary_bytes"] = (work / "program").stat().st_size
    ledger["evidence"] = [{"path": str(path), "bytes": path.stat().st_size, "sha256": sha(path)}
                          for path in work.rglob("*") if path.is_file()]
ledger["compiler_stable"] = sha(baseline["compiler"]) == baseline["compiler_sha256"]
ledger["complete"] = True
(out / "retention-build.json").write_text(json.dumps(ledger, indent=2) + "\n")
raise SystemExit(result.returncode)

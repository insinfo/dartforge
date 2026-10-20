set -euo pipefail
snapshot=$(cat /tmp/forge-current-desktop-linux-snapshot)
runuser -l isaque -c "cd '$snapshot' && PATH=/opt/llvm-22.1.8/bin:\$PATH LLVM_SYS_221_PREFIX=/opt/llvm-22.1.8 DARTFORGE_LLVM_DIR=/opt/llvm-22.1.8 DARTFORGE_RUNTIME_NO_PRECOMPILE=0 CARGO_BUILD_JOBS=1 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_RELEASE_OPT_LEVEL=1 CARGO_PROFILE_RELEASE_DEBUG=0 CARGO_PROFILE_RELEASE_LTO=false cargo build --release --locked -p dartforge-cli --features jit --target-dir /home/isaque/projects/dartforge/target/native-validation" > /tmp/forge-current-desktop-linux-build.log 2>&1
python3 - <<'PY'
from pathlib import Path
import shutil,os,hashlib,json
base=Path('/home/isaque/projects/dartforge/target/native-validation/release');old=base/'dartforge';new=base/'dartforge-current-desktop';stage=base/'dartforge-current-desktop.staging';shutil.copy2(old,stage);os.chown(stage,1000,1000);stage.replace(new)
report=dict(binary=str(new),sha256=hashlib.sha256(new.read_bytes()).hexdigest(),bytes=new.stat().st_size)
Path('/media/psf/ssd_dock/MyRustProjects/dartforge/target/current-desktop-validation/linux-compiler.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
PY

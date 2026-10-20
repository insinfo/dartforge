set -euo pipefail
snapshot=$(mktemp -d /tmp/dartforge-workspace-format-source.XXXXXX)
python3 - "$snapshot" <<'PYCODE'
import pathlib,tarfile,hashlib,sys
archive=pathlib.Path('/media/psf/ssd_dock/MyRustProjects/dartforge/target/workspace-format-validation/source-snapshot.tar.gz')
assert hashlib.sha256(archive.read_bytes()).hexdigest() == 'd1d5957c984eafef5d55d704cf0c373896d9c0e37d9fbd416375e43ead13b594'
with tarfile.open(archive) as source:source.extractall(sys.argv[1],filter='data')
PYCODE
cat > "$snapshot/crates/emit_native/examples/export_current_runtime.rs" <<'RUST'
//! Export the native runtime bundle of this frozen validation snapshot.
fn main() { dartforge_emit_native::cross::export(&std::path::PathBuf::from(std::env::args_os().nth(1).unwrap())).unwrap(); }
RUST
chown -R isaque:isaque "$snapshot"
printf '%s\n' "$snapshot" > /tmp/forge-workspace-format-linux-snapshot
runuser -l isaque -c "cd '$snapshot' && PATH=/opt/llvm-22.1.8/bin:\$PATH LLVM_SYS_221_PREFIX=/opt/llvm-22.1.8 DARTFORGE_LLVM_DIR=/opt/llvm-22.1.8 CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_RELEASE_LTO=false cargo build --locked -p dartforge-emit-native --example export_current_runtime --target-dir /home/isaque/projects/dartforge/target/native-validation" > /tmp/forge-workspace-format-linux-build-final.log 2>&1
runuser -l isaque -c "PATH=/opt/llvm-22.1.8/bin:\$PATH DARTFORGE_LLVM_DIR=/opt/llvm-22.1.8 /home/isaque/projects/dartforge/target/native-validation/debug/examples/export_current_runtime '$snapshot/bundle'"
mkdir -p /media/psf/ssd_dock/MyRustProjects/dartforge/target/cross-bundles/workspace-format/linux-arm64
cp -R "$snapshot/bundle/." /media/psf/ssd_dock/MyRustProjects/dartforge/target/cross-bundles/workspace-format/linux-arm64/
cat "$snapshot/bundle/target.json"

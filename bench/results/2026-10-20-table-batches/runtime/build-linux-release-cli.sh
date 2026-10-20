set -euo pipefail
snapshot=$(cat /tmp/forge-batch-tables-linux-snapshot)
runuser -l isaque -c "cd '$snapshot' && PATH=/opt/llvm-22.1.8/bin:\$PATH LLVM_SYS_221_PREFIX=/opt/llvm-22.1.8 DARTFORGE_LLVM_DIR=/opt/llvm-22.1.8 DARTFORGE_RUNTIME_NO_PRECOMPILE=0 CARGO_BUILD_JOBS=1 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_RELEASE_OPT_LEVEL=1 CARGO_PROFILE_RELEASE_DEBUG=0 CARGO_PROFILE_RELEASE_LTO=false cargo build --release --locked -p dartforge-cli --features jit --target-dir /home/isaque/projects/dartforge/target/native-validation" > /tmp/forge-batch-tables-linux-release-cli-build.log 2>&1
cp /home/isaque/projects/dartforge/target/native-validation/release/dartforge /home/isaque/projects/dartforge/target/native-validation/release/dartforge-batch-tables
chown isaque:isaque /home/isaque/projects/dartforge/target/native-validation/release/dartforge-batch-tables
sha256sum /home/isaque/projects/dartforge/target/native-validation/release/dartforge-batch-tables

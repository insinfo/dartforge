source /Volumes/ssd_dock/tools/env.zsh
export CARGO_TARGET_DIR="$PWD/target/jit-readiness-build" TMPDIR="$PWD/target/jit-arc-readiness"
export LLVM_SYS_221_PREFIX=/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64 DARTFORGE_LLVM_DIR=/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64
export DARTFORGE_CLANG=/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang DARTFORGE_LLVM_ZSTD_LIBRARY="$PWD/target/jit-arc-readiness/zstd/libzstd.a"
export DARTFORGE_TEST_SDK_LIB=/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib DARTFORGE_SDK_LIB=/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib DARTFORGE_EXCEPTIONS=check
export CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk
export RUSTFLAGS='-C link-arg=-fuse-ld=lld -C link-arg=-isysroot -C link-arg=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk'

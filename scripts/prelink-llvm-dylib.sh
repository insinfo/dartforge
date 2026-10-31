#!/usr/bin/env bash
# prelink-llvm-dylib.sh PREFIX OUTPUT: links the static archives of the LLVM at
# PREFIX into one shared library OUTPUT (libLLVM-22.dylib), for
# DARTFORGE_LLVM_DYLIB (crates/llvm/build.rs). Rerun it if OUTPUT moves.
#
# The official macOS package only has bitcode archives: a statically linked
# executable runs LLVM's whole LTO again, and the CI's emitter and JIT tests
# link dozens of them. This runs that LTO once. Every component llvm-config lists
# is force-loaded, except LTO, DTLTO and Extensions, which reference the Polly
# plugin the package does not ship; they come in only where something uses them.
# The SDK is the first that the package's ld64.lld can read (newer SDKs carry
# text stubs it does not parse), as scripts/cargo-macos-llvm.py probes it.
set -euo pipefail
prefix=$1
output=$2
clang="$prefix/bin/clang"
lld="$prefix/bin/ld64.lld"

sdk=""
default=$(xcrun --show-sdk-path)
probe=$(mktemp -d)
for candidate in "$default" $(ls -d "$(dirname "$default")"/MacOSX*.sdk 2>/dev/null | sort -r); do
  if echo 'int main(void) { return 0; }' |
    "$clang" --ld-path="$lld" -isysroot "$candidate" -x c - -o "$probe/a.out" 2>/dev/null; then
    sdk=$candidate
    break
  fi
done
rm -rf "$probe"
if [ -z "$sdk" ]; then
  echo "no macOS SDK that $lld can read" >&2
  exit 1
fi

forced=()
lazy=()
for library in $("$prefix/bin/llvm-config" --link-static --libs); do
  archive="$prefix/lib/lib${library#-l}.a"
  [ -f "$archive" ] || continue
  case ${library#-l} in
    LLVMLTO | LLVMDTLTO | LLVMExtensions) lazy+=("$archive") ;;
    *) forced+=("-Wl,-force_load,$archive") ;;
  esac
done
system=()
for library in $("$prefix/bin/llvm-config" --link-static --system-libs); do
  case $library in
    -l*) system+=("$library") ;;
    # The package names the zstd archive of its build machine: the one at
    # DARTFORGE_LLVM_ZSTD_LIBRARY, or Homebrew's, here.
    *zstd*) system+=("${DARTFORGE_LLVM_ZSTD_LIBRARY:-$(brew --prefix zstd)/lib/libzstd.a}") ;;
    *) system+=("$library") ;;
  esac
done

mkdir -p "$(dirname "$output")"
# Its install name is its absolute path: the executables of every crate that
# links it find it without an rpath (a build script's link arguments reach only
# its own package's targets).
output=$(cd "$(dirname "$output")" && pwd)/$(basename "$output")
start=$SECONDS
"$clang" -dynamiclib -isysroot "$sdk" --ld-path="$lld" -mmacosx-version-min=11.0 \
  -o "$output" "${forced[@]}" "${lazy[@]}" "${system[@]}" -ledit -lc++ \
  -Wl,-install_name,"$output" \
  -Wl,--thinlto-jobs="$(sysctl -n hw.ncpu)" 2> >(grep -v 'warning:' >&2)
echo "prelinked $output with $sdk in $((SECONDS - start)) s"

#!/bin/sh
# Ordinary Rust links need only the existing system C compiler. Inspect LLVM
# archives in Python only when they can require a matching bitcode reader.
llvm=false
value=
for argument do
    if [ "$value" = skip ]; then
        value=
        continue
    fi
    if [ "$value" = library ]; then
        case "$argument" in LLVM*) llvm=true ;; esac
        value=
        continue
    fi
    case "$argument" in
        -o|-L|-isysroot|--sysroot) value=skip ;;
        -l) value=library ;;
        -fuse-ld=*|-fuse-ld|-Wl,-ld_classic) exec cc "$@" ;;
        *libdartforge_llvm-*.rlib|*libLLVM*.a|-lLLVM*) llvm=true ;;
        @*)
            if grep -q -- LLVM "${argument#@}" 2>/dev/null ||
               grep -q -- dartforge_llvm "${argument#@}" 2>/dev/null ||
               grep -q -- @ "${argument#@}" 2>/dev/null; then
                llvm=true
            fi ;;
    esac
done
if [ "$llvm" = false ]; then
    exec cc "$@"
fi
script_dir=$(CDPATH= cd -P "$(dirname "$0")" && pwd) || exit 2
exec python3 "$script_dir/rust-linker-macos.py" "$@"

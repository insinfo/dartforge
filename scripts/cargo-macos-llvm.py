#!/usr/bin/env python3
"""Run Cargo with the selected LLVM's Mach-O linker and a compatible macOS SDK.

Usage: DARTFORGE_LLVM_DIR=/path/to/llvm python3 scripts/cargo-macos-llvm.py test
All arguments are forwarded to Cargo. Configuration affects only this process.
An explicit SDKROOT is validated and never replaced with another SDK.
"""
import os
from pathlib import Path
import platform
import re
import shlex
import subprocess
import sys
import tempfile


def llvm_prefix(environment, run=subprocess.run):
    """Find LLVM 22.1 using explicit configuration or llvm-sys's PATH order."""
    explicit = environment.get('DARTFORGE_LLVM_DIR') or environment.get('LLVM_SYS_221_PREFIX')
    if explicit:
        return Path(explicit).resolve()
    for binary in ('llvm-config-22', 'llvm-config22', 'llvm22-config',
                   'llvm-config-22.1', 'llvm-config221', 'llvm-config'):
        try:
            version = run([binary, '--version'], capture_output=True, text=True, env=environment)
            if version.returncode or not re.match(r'^22\.1(?:\.|$)', version.stdout.strip()):
                continue
            prefix = run([binary, '--prefix'], capture_output=True, text=True, env=environment)
            if prefix.returncode == 0 and prefix.stdout.strip():
                return Path(prefix.stdout.strip()).resolve()
        except OSError:
            continue
    raise ValueError('LLVM 22.1 was not found. Set DARTFORGE_LLVM_DIR or expose llvm-config on PATH.')


def prepare(environment, run=subprocess.run):
    """Return a child environment after a real linker/SDK compatibility probe."""
    environment = dict(environment)
    prefix = llvm_prefix(environment, run)
    pinned = environment.get('DARTFORGE_MACOS_SDK_ROOT')
    if pinned:
        if environment.get('SDKROOT') and Path(environment['SDKROOT']).resolve() != Path(pinned).resolve():
            raise ValueError('DARTFORGE_MACOS_SDK_ROOT conflicts with SDKROOT.')
        environment['SDKROOT'] = pinned
    clang = prefix / 'bin/clang'
    linker = prefix / 'bin/ld64.lld'
    for tool in (clang, linker):
        if not os.access(tool, os.X_OK):
            raise ValueError(f'Required executable is missing: {tool}')

    if environment.get('SDKROOT'):
        candidates = [Path(environment['SDKROOT'])]
    else:
        result = run(['xcrun', '--show-sdk-path'], capture_output=True, text=True,
                     env=environment)
        if result.returncode:
            raise ValueError(f'Cannot locate macOS SDK: {result.stderr.strip()}')
        default = Path(result.stdout.strip())
        # Try the active SDK first, then installed siblings. Probe instead of
        # assuming that an SDK version's text stub format is supported by LLD.
        candidates = [default, *sorted(default.parent.glob('MacOSX*.sdk'), reverse=True)]

    failures = []
    seen = set()
    with tempfile.TemporaryDirectory(prefix='dartforge-link-probe-') as temporary:
        output = Path(temporary) / 'probe'
        for sdk in candidates:
            sdk = sdk.resolve()
            if sdk in seen:
                continue
            seen.add(sdk)
            result = run(
                [str(clang), f'-fuse-ld={linker}', '-isysroot', str(sdk),
                 '-x', 'c', '-', '-o', str(output)],
                input='int main(void) { return 0; }\n', capture_output=True,
                text=True, env=environment)
            if result.returncode == 0:
                environment['SDKROOT'] = str(sdk)
                break
            failures.append(f'{sdk}: {result.stderr.strip()[-2000:]}')
        else:
            raise ValueError('No compatible macOS SDK found.\n' + '\n'.join(failures))

    # Cargo gives encoded flags precedence over RUSTFLAGS. Preserve the same
    # precedence and keep paths containing spaces as one rustc argument.
    encoded = environment.get('CARGO_ENCODED_RUSTFLAGS')
    flags = ((encoded.split('\x1f') if encoded else []) if encoded is not None
             else shlex.split(environment.get('RUSTFLAGS', '')))
    flags.extend(['-C', f'link-arg=-fuse-ld={linker}'])
    environment['CARGO_ENCODED_RUSTFLAGS'] = '\x1f'.join(flags)
    environment['DARTFORGE_LLVM_DIR'] = str(prefix)
    environment['LLVM_SYS_221_PREFIX'] = str(prefix)
    return environment


def main():
    if platform.system() != 'Darwin':
        print('This helper requires macOS.', file=sys.stderr)
        return 2
    if len(sys.argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    try:
        environment = prepare(os.environ)
    except (ValueError, OSError) as error:
        print(error, file=sys.stderr)
        return 2
    print(f"Cargo LLVM linker: {environment['DARTFORGE_LLVM_DIR']}/bin/ld64.lld; "
          f"SDKROOT={environment['SDKROOT']}", file=sys.stderr)
    return subprocess.call(['cargo', *sys.argv[1:]], env=environment)


if __name__ == '__main__':
    sys.exit(main())

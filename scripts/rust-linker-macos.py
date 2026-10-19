#!/usr/bin/env python3
"""Select matching LLD only for LLVM archives that contain raw bitcode."""
import importlib.util
import os
from pathlib import Path
import shlex
import subprocess
import sys

spec = importlib.util.spec_from_file_location(
    'cargo_macos_llvm', Path(__file__).with_name('cargo-macos-llvm.py'))
helper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)

BITCODE_MAGIC = (b'BC\xc0\xde', b'\xde\xc0\x17\x0b')


def contains_bitcode(path):
    """Read archive headers and member magic, without loading LLVM archives."""
    with path.open('rb') as stream:
        magic = stream.read(8)
        if magic[:4] in BITCODE_MAGIC:
            return True
        if magic != b'!<arch>\n':
            raise ValueError(f'Unsupported LLVM archive: {path}')
        end = os.fstat(stream.fileno()).st_size
        while stream.tell() < end:
            header = stream.read(60)
            if len(header) != 60 or header[58:] != b'`\n':
                raise ValueError(f'Invalid LLVM archive header: {path}')
            size = int(header[48:58])
            start = stream.tell()
            if size < 0 or start + size > end:
                raise ValueError(f'Invalid LLVM archive member size: {path}')
            name = header[:16].decode('ascii').strip()
            extended = int(name[3:]) if name.startswith('#1/') else 0
            if extended < 0 or extended > size:
                raise ValueError(f'Invalid LLVM archive member name: {path}')
            stream.seek(extended, 1)
            if stream.read(min(4, size - extended)) in BITCODE_MAGIC:
                return True
            stream.seek(start + size + size % 2)
    return False


def expand_response_files(arguments, active=()):
    """Expand clang response files for inspection; preserve originals for exec."""
    expanded = []
    for argument in arguments:
        if not argument.startswith('@'):
            expanded.append(argument)
            continue
        path = Path(argument[1:]).resolve()
        if path in active or len(active) >= 16:
            raise ValueError(f'Recursive linker response file: {path}')
        expanded.extend(expand_response_files(shlex.split(path.read_text()), (*active, path)))
    return expanded


def explicit_linker(arguments):
    return any(arg == '-fuse-ld' or arg.startswith('-fuse-ld=') or arg == '-Wl,-ld_classic'
               for arg in arguments)


def llvm_archives(arguments):
    directories = []
    libraries = []
    for index, argument in enumerate(arguments):
        if argument == '-L' and index + 1 < len(arguments):
            directories.append(Path(arguments[index + 1]))
        elif argument.startswith('-L'):
            directories.append(Path(argument[2:]))
        elif argument.startswith('-lLLVM'):
            libraries.append(argument[2:])
        elif argument == '-l' and index + 1 < len(arguments) and arguments[index + 1].startswith('LLVM'):
            libraries.append(arguments[index + 1])
    skip = False
    for argument in arguments:
        if skip:
            skip = False
            continue
        if argument in ('-o', '-L', '-l', '-isysroot', '--sysroot'):
            skip = True
            continue
        if argument.startswith('-Wl,-force_load,'):
            argument = argument[len('-Wl,-force_load,'):]
        name = Path(argument).name
        if ((name.startswith('libdartforge_llvm-') and name.endswith('.rlib')) or
                (name.startswith('libLLVM') and name.endswith('.a'))):
            yield Path(argument)
    # Rust links the LLVM crate's own tests with -lLLVM* instead of bundling
    # those libraries in another rlib. Honor normal per-directory dylib lookup.
    for library in libraries:
        for directory in directories:
            if any((directory / f'lib{library}{suffix}').is_file() for suffix in ('.dylib', '.tbd')):
                break
            archive = directory / f'lib{library}.a'
            if archive.is_file():
                yield archive
                break


def sdk_arguments(arguments):
    roots = []
    for index, argument in enumerate(arguments):
        if argument in ('-isysroot', '--sysroot'):
            if index + 1 >= len(arguments):
                raise ValueError(f'Missing SDK path after {argument}')
            roots.append((arguments[index + 1], False))
        elif argument.startswith('--sysroot='):
            roots.append((argument.split('=', 1)[1], False))
        elif argument.startswith('-isysroot'):
            roots.append((argument[len('-isysroot'):], False))
        elif argument.startswith('-Wl,-syslibroot,'):
            roots.append((argument[len('-Wl,-syslibroot,'):], True))
        elif argument == '-Xlinker' and index + 1 < len(arguments) and arguments[index + 1] == '-syslibroot':
            value = index + 2
            if value < len(arguments) and arguments[value] == '-Xlinker':
                value += 1
            if value >= len(arguments):
                raise ValueError('Missing SDK path after -Xlinker -syslibroot')
            roots.append((arguments[value], True))
    return roots


def active_sdk(environment, run):
    try:
        result = run(['xcrun', '--show-sdk-path'], capture_output=True, text=True,
                     env={key: value for key, value in environment.items() if key != 'SDKROOT'})
    except OSError:
        return None
    return Path(result.stdout.strip()).resolve() if result.returncode == 0 and result.stdout.strip() else None


def command(arguments, environment, run=subprocess.run, prepare=helper.prepare):
    expanded = expand_response_files(arguments)
    if explicit_linker(expanded) or not any(contains_bitcode(path) for path in llvm_archives(expanded)):
        return ['cc', *arguments], environment
    environment = dict(environment)
    roots = sdk_arguments(expanded)
    pinned = environment.get('DARTFORGE_MACOS_SDK_ROOT')
    if pinned:
        if (environment.get('SDKROOT') and Path(environment['SDKROOT']).resolve() != Path(pinned).resolve()
                and Path(environment['SDKROOT']).resolve() != active_sdk(environment, run)):
            raise ValueError('DARTFORGE_MACOS_SDK_ROOT conflicts with SDKROOT.')
        if roots and Path(roots[-1][0]).resolve() != Path(pinned).resolve():
            raise ValueError('DARTFORGE_MACOS_SDK_ROOT conflicts with the SDK path in the linker arguments.')
        environment['SDKROOT'] = pinned
    elif environment.get('SDKROOT'):
        # rustc supplies the active SDKROOT even when Cargo's caller did not.
        # The default SDK is therefore a hint, not evidence of a caller pin.
        # A distinct SDKROOT remains explicit; the named project variable pins
        # the active SDK too, without relying on this otherwise ambiguous value.
        if Path(environment['SDKROOT']).resolve() == active_sdk(environment, run):
            environment.pop('SDKROOT')
    if (roots and environment.get('SDKROOT') and
            Path(roots[-1][0]).resolve() != Path(environment['SDKROOT']).resolve()):
        raise ValueError('SDKROOT conflicts with the SDK path in the linker arguments.')
    if roots and not environment.get('SDKROOT'):
        root, explicit = roots[-1]
        if not explicit:
            explicit = Path(root).resolve() != active_sdk(environment, run)
        if explicit:
            environment['SDKROOT'] = root
    prepared = prepare(environment, run)
    prefix = Path(prepared['DARTFORGE_LLVM_DIR'])
    # Clang takes the final driver SDK option. Original response files and
    # unrelated rustc arguments stay intact, including paths containing spaces.
    return [str(prefix / 'bin/clang'), *arguments, '-isysroot', prepared['SDKROOT'],
            f'-fuse-ld={prefix}/bin/ld64.lld'], prepared


def main():
    try:
        arguments, environment = command(sys.argv[1:], os.environ)
        os.execvpe(arguments[0], arguments, environment)
    except (ValueError, OSError) as error:
        print(f'DartForge macOS linker: {error}', file=sys.stderr)
        return 2


if __name__ == '__main__':
    sys.exit(main())

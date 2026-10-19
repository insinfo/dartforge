"""Exercise the actual build script with real and invalid LLVM installations.

Usage: python3 verify_process.py /path/to/llvm output-directory
The process fixture uses POSIX shell scripts; Windows behavior is not certified.
"""
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile


def main():
    if os.name == 'nt':
        raise SystemExit('The process fixture requires a POSIX host.')
    prefix = Path(sys.argv[1]).resolve()
    output = Path(sys.argv[2]).resolve()
    output.mkdir(parents=True, exist_ok=True)
    repository = Path(__file__).resolve().parents[3]
    results = {}
    with tempfile.TemporaryDirectory(prefix='llvm selection ') as temporary:
        temporary = Path(temporary)
        binary = temporary / 'build-script'
        subprocess.run(['rustc', '--edition=2024', str(repository / 'crates/llvm/build.rs'),
                        '-o', str(binary)], check=True)
        fake = temporary / 'LLVM 21'
        (fake / 'bin').mkdir(parents=True)
        tool = fake / 'bin/llvm-config'
        tool.write_text('#!/bin/sh\nprintf "21.1.8\\n"\n')
        tool.chmod(0o755)
        environment = dict(os.environ)
        for name in ('DARTFORGE_LLVM_DIR', 'LLVM_SYS_221_PREFIX'):
            environment.pop(name, None)
        environment['PATH'] = str(prefix / 'bin') + os.pathsep + environment['PATH']
        environment['CARGO_CFG_TARGET_OS'] = 'macos' if sys.platform == 'darwin' else 'linux'
        environment['CARGO_CFG_TARGET_ARCH'] = {'arm64': 'aarch64', 'AMD64': 'x86_64'}.get(
            platform.machine(), platform.machine())
        fixtures = [
            ('path-only', {}, 0, None),
            ('explicit', {'DARTFORGE_LLVM_DIR': str(prefix)}, 0, None),
            ('legacy', {'LLVM_SYS_221_PREFIX': str(prefix)}, 0, None),
            ('broken-explicit', {'DARTFORGE_LLVM_DIR': str(temporary / 'missing'),
                                 'LLVM_SYS_221_PREFIX': str(prefix)}, 101,
             'cannot validate selected LLVM installation'),
            ('wrong-version', {'DARTFORGE_LLVM_DIR': str(fake)}, 101,
             'requires LLVM 22.1.x'),
            ('missing', {'PATH': str(temporary)}, 101, 'LLVM 22.1 was not found'),
        ]
        for name, overrides, expected, message in fixtures:
            result = subprocess.run([str(binary)], env=environment | overrides,
                                    stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
            (output / (name + '.log')).write_text(result.stdout)
            assert result.returncode == expected, (name, result.returncode, result.stdout)
            if message:
                assert message in result.stdout, (name, result.stdout)
            results[name] = {'exit': result.returncode, 'expected_exit': expected}
    (output / 'process-results.json').write_text(json.dumps(results, indent=2) + '\n')
    print(json.dumps(results, indent=2))


if __name__ == '__main__':
    main()

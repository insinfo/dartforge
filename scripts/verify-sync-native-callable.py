#!/usr/bin/env python3
"""Execute the synchronous callback owner-delivery and cancellation oracle."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
PACKAGE = ROOT / 'packages/dartforge'
PROBE = PACKAGE / 'test/sync_native_callable_probe.dart'
HELPER = PACKAGE / 'test/native/sync_native_callable_probe.c'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', type=Path, default=ROOT / 'target/p4/release/dartforge')
    parser.add_argument('--output', type=Path, default=ROOT / 'target/sync-native-callable')
    parser.add_argument('--dart', default=shutil.which('dart'))
    args = parser.parse_args()
    if sys.platform not in ('darwin', 'linux'):
        parser.error('The pthread test helper requires macOS or Linux')
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    compiler = args.compiler.resolve()
    sources = [PROBE, HELPER, Path(__file__).resolve(),
               PACKAGE / 'lib/src/sync_native_callable.dart']
    hashes = {str(path.relative_to(ROOT)): digest(path) for path in sources}
    sdk = Path(args.dart).resolve().parent.parent / 'lib'
    clang = os.environ['DARTFORGE_CLANG']
    environment = dict(os.environ, TMPDIR=str(output),
                       DARTFORGE_EFFECTS='check', DARTFORGE_ARC_CHECK='1')
    for name in ('DARTFORGE_KEEP_SYMBOLS', 'DARTFORGE_GC_STRESS',
                 'DARTFORGE_GC_SCHEDULE', 'DARTFORGE_ARC_PREPARE',
                 'DARTFORGE_ARC_PREPARED'):
        environment.pop(name, None)
    ledger = {'compiler_sha256': digest(compiler), 'sources': hashes,
              'commands': [], 'runs': [], 'realtime_certified': False,
              'nested_event_turns_supported': False, 'inputs_stable': False}

    def run(command, label, env=environment):
        command = list(map(str, command))
        result = subprocess.run(command, cwd=ROOT, env=env,
                                capture_output=True, timeout=300)
        (output / (label + '.stdout')).write_bytes(result.stdout)
        (output / (label + '.stderr')).write_bytes(result.stderr)
        ledger['commands'].append({'label': label, 'command': command,
                                   'exit_code': result.returncode})
        if result.returncode:
            raise RuntimeError(f'{label}: exit {result.returncode}; inspect {output}')
        return result

    run([args.dart, 'analyze', PACKAGE], 'analyze')
    helper = output / ('helper.dylib' if sys.platform == 'darwin' else 'helper.so')
    command = [clang, '-Wall', '-Wextra', '-Werror']
    if sys.platform == 'darwin':
        mac_sdk = run(['xcrun', '--show-sdk-path'], 'mac-sdk').stdout.decode().strip()
        command += ['-isysroot', mac_sdk, '-dynamiclib']
    else:
        command += ['-shared', '-fPIC', '-pthread']
    run([*command, HELPER, '-o', helper], 'helper-build')
    ledger['helper_sha256'] = digest(helper)
    fallback = b'sync callback fallback validated\n'
    result = run([args.dart, PROBE], 'vm')
    if result.stdout != fallback or result.stderr:
        raise RuntimeError('VM fallback mismatch')
    official = output / 'official'
    run([args.dart, 'compile', 'exe', PROBE, '-o', official], 'official-build')
    result = run([official], 'official')
    if result.stdout != fallback or result.stderr:
        raise RuntimeError('Official AOT fallback mismatch')
    packages = PACKAGE / '.dart_tool/package_config.json'
    for mode in ('tracing', 'arc', 'prepared'):
        current = dict(environment)
        if mode == 'prepared':
            current['DARTFORGE_ARC_PREPARED'] = '1'
        binary = output / mode
        run([compiler, 'compile-native', PROBE, '--sdk', sdk,
             '--packages', packages, '--optimize', '--exceptions=tables',
             '--roots=shadow', '--stack-trace=none', '--dartforge-contracts=verify',
             '--memory=' + ('tracing' if mode == 'tracing' else 'arc'),
             '-o', binary], mode + '-build', current)
        manifest_path = Path(str(binary) + '.contracts.json')
        manifest = json.loads(manifest_path.read_text())
        if manifest['mode'] != 'verify' or manifest['certified'] is not False:
            raise RuntimeError('Unexpected contract policy')
        if Path(manifest['artifact']['path']).resolve() != binary:
            raise RuntimeError('Manifest belongs to another binary')
        for stress in (False, True):
            runtime = dict(current)
            if stress:
                runtime['DARTFORGE_GC_STRESS'] = '1'
            label = mode + ('-stress' if stress else '-normal')
            result = run([binary, helper], label, runtime)
            if result.stdout != b'sync callback owner delivery and cancellation validated\n' or result.stderr:
                raise RuntimeError(f'{label}: callback oracle mismatch')
            ledger['runs'].append({'mode': mode, 'gc_stress': stress,
                                   'prepared_environment': current.get('DARTFORGE_ARC_PREPARED'),
                                   'binary_sha256': digest(binary),
                                   'manifest_sha256': digest(manifest_path),
                                   'stdout': result.stdout.decode()})
            print(label + ' PASS', flush=True)
    if digest(compiler) != ledger['compiler_sha256'] or any(
            digest(ROOT / name) != value for name, value in hashes.items()):
        raise RuntimeError('Validation inputs changed during execution')
    ledger['inputs_stable'] = True
    (output / 'validation.json').write_text(json.dumps(ledger, indent=2) + '\n')
    print('Six native callback modes verified', flush=True)


if __name__ == '__main__':
    main()

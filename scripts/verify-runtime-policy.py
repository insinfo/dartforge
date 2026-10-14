#!/usr/bin/env python3
"""Verify bounded collection, scheduler snapshots and owning OS thread identity."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
PACKAGE = ROOT / 'packages/dartforge'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', type=Path, default=ROOT / 'target/p4/release/dartforge')
    parser.add_argument('--output', type=Path, default=ROOT / 'target/runtime-policy-validation')
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    compiler = args.compiler.resolve()
    dart = Path(shutil.which('dart')).resolve()
    sdk = dart.parent.parent / 'lib'
    probe = PACKAGE / 'test/runtime_policy_probe.dart'
    inputs = [probe, Path(__file__).resolve(), PACKAGE / 'lib/src/gc_policy.dart',
              PACKAGE / 'lib/src/runtime_state.dart', PACKAGE / 'lib/src/native_pump.dart']
    hashes = {str(p.relative_to(ROOT)): sha(p) for p in inputs}
    environment = dict(os.environ, TMPDIR=str(output), DARTFORGE_EFFECTS='check', DARTFORGE_ARC_CHECK='1')
    for name in ('DARTFORGE_GC_STRESS', 'DARTFORGE_GC_SCHEDULE', 'DARTFORGE_GC_OFF',
                 'DARTFORGE_ARC_PREPARE', 'DARTFORGE_ARC_PREPARED'):
        environment.pop(name, None)
    ledger = {'compiler_sha256': sha(compiler), 'sources': hashes, 'runs': [],
              'commands': [], 'performance_measurement': False, 'inputs_stable': False}

    def run(command, label, env=environment):
        command = list(map(str, command))
        result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, timeout=300)
        (output / (label + '.stdout')).write_bytes(result.stdout)
        (output / (label + '.stderr')).write_bytes(result.stderr)
        ledger['commands'].append({'label': label, 'command': command, 'exit_code': result.returncode})
        if result.returncode:
            raise RuntimeError(f'{label}: exit {result.returncode}; inspect {output}')
        return result

    fallback = b'runtime policy fallback validated\n'
    for label, command in [('vm', [dart, probe])]:
        result = run(command, label)
        if result.stdout != fallback or result.stderr:
            raise RuntimeError(label + ': fallback mismatch')
    official = output / 'official'
    run([dart, 'compile', 'exe', probe, '-o', official], 'official-build')
    result = run([official], 'official')
    if result.stdout != fallback or result.stderr:
        raise RuntimeError('Official fallback mismatch')
    for mode in ('tracing', 'arc', 'prepared'):
        current = dict(environment)
        if mode == 'prepared':
            current['DARTFORGE_ARC_PREPARED'] = '1'
        binary = output / mode
        run([compiler, 'compile-native', probe, '--sdk', sdk, '--packages',
             PACKAGE / '.dart_tool/package_config.json', '--optimize', '--exceptions=tables',
             '--roots=shadow', '--stack-trace=none', '--dartforge-contracts=verify',
             '--memory=' + ('tracing' if mode == 'tracing' else 'arc'), '-o', binary],
            mode + '-build', current)
        manifest = Path(str(binary) + '.contracts.json')
        report = json.loads(manifest.read_text())
        if report['certified'] is not False or report['mode'] != 'verify':
            raise RuntimeError('Unexpected contract certification')
        for stress in (False, True):
            runtime = dict(current)
            if stress:
                runtime['DARTFORGE_GC_STRESS'] = '1'
            label = mode + ('-stress' if stress else '-normal')
            result = run([binary], label, runtime)
            if result.stdout != b'runtime deadline, thread identity and bounded GC policy validated\n' or result.stderr:
                raise RuntimeError(label + ': native oracle mismatch')
            ledger['runs'].append({'mode': mode, 'stress': stress,
                                   'prepared_environment': current.get('DARTFORGE_ARC_PREPARED'),
                                   'binary_sha256': sha(binary), 'manifest_sha256': sha(manifest)})
            print(label + ' PASS', flush=True)
    if sha(compiler) != ledger['compiler_sha256'] or any(
            sha(ROOT / name) != value for name, value in hashes.items()):
        raise RuntimeError('Inputs changed during validation')
    ledger['inputs_stable'] = True
    (output / 'validation.json').write_text(json.dumps(ledger, indent=2) + '\n')


if __name__ == '__main__':
    main()

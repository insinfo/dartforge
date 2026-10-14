#!/usr/bin/env python3
"""Compare portable resource probes on the VM, official AOT and native AOT.

These are functional checks, not performance measurements. Temporary artifacts
remain under the chosen SSD output directory. Existing package configuration is
used without package resolution or changes to the framework repository.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess

ROOT = Path(__file__).resolve().parents[1]
PACKAGE = ROOT / 'packages/dartforge'


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', type=Path,
                        default=ROOT / 'target/p4/release/dartforge')
    parser.add_argument('--dart', default=os.environ.get('DART', 'dart'))
    parser.add_argument('--probe', action='append', required=True,
                        help='Probe basename under packages/dartforge/test')
    parser.add_argument('--output', type=Path,
                        default=ROOT / 'target/package-resources')
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    compiler = args.compiler.resolve()
    config = PACKAGE / '.dart_tool/package_config.json'
    sources = []
    for name in args.probe:
        source = PACKAGE / 'test' / name
        if source.parent != PACKAGE / 'test' or source.suffix != '.dart':
            raise ValueError('Probes must be Dart basenames in the package test directory')
        if not source.is_file():
            raise ValueError(f'Missing probe: {source}')
        sources.append(source)
    inputs = [config, PACKAGE / 'pubspec.yaml', *sorted((PACKAGE / 'lib').rglob('*.dart')), *sources]
    hashes = {str(path.relative_to(ROOT)): sha256(path) for path in inputs}
    base = os.environ.copy()
    base.update(TMPDIR=str(output), DARTFORGE_EFFECTS='check', DARTFORGE_ARC_CHECK='1')
    for key in ('DARTFORGE_GC_STRESS', 'DARTFORGE_ARC_PREPARED'):
        base.pop(key, None)
    metadata = {'scope': 'Functional parity; no performance claim',
                'platform': platform.platform(), 'compiler': str(compiler),
                'compiler_sha256': sha256(compiler), 'inputs_sha256': hashes,
                'commands': [], 'runs': [], 'artifacts': [],
                'inputs_stable_through_run': False}
    ledger = output / 'validation.json'

    def save():
        ledger.write_text(json.dumps(metadata, indent=2) + '\n')

    def run(command, label, environment=base, timeout=180):
        completed = subprocess.run([str(value) for value in command], cwd=ROOT,
                                   env=environment, capture_output=True, timeout=timeout)
        (output / (label + '.stdout')).write_bytes(completed.stdout)
        (output / (label + '.stderr')).write_bytes(completed.stderr)
        metadata['commands'].append({'label': label, 'command': [str(v) for v in command],
                                     'exit_code': completed.returncode})
        save()
        if completed.returncode:
            raise RuntimeError(f'{label} exited {completed.returncode}; inspect {output}')
        return completed

    metadata['sdk'] = run([args.dart, '--version'], 'sdk').stdout.decode().strip()
    for source in sources:
        stem = source.stem
        reference = run([args.dart, '--packages=' + str(config), 'run', source], stem + '-vm')
        if reference.stderr:
            raise RuntimeError(f'{stem}: VM reference emitted stderr')
        official = output / (stem + '.official')
        run([args.dart, 'compile', 'exe', '--packages=' + str(config), source,
             '-o', official], stem + '-official-build')
        actual = run([official], stem + '-official')
        if (actual.stdout, actual.stderr) != (reference.stdout, reference.stderr):
            raise RuntimeError(f'{stem}: upstream AOT differs from VM')
        for mode in ('tracing', 'arc', 'prepared'):
            environment = dict(base)
            if mode == 'prepared':
                environment['DARTFORGE_ARC_PREPARED'] = '1'
            binary = output / (stem + '.' + mode)
            label = stem + '-' + mode
            run([compiler, 'compile-native', source, '--packages', config,
                 '--optimize', '--exceptions', 'tables', '--roots', 'shadow',
                 '--stack-trace', 'none', '--memory', 'tracing' if mode == 'tracing' else 'arc',
                 '--dartforge-contracts=verify', '-o', binary], label + '-build', environment)
            sidecar = Path(str(binary) + '.contracts.json')
            manifest = json.loads(sidecar.read_text())
            if manifest['mode'] != 'verify' or manifest['certified'] is not False:
                raise RuntimeError(f'{label}: unexpected contract policy/certification')
            binary_hash, sidecar_hash = sha256(binary), sha256(sidecar)
            metadata['artifacts'].append({'probe': source.name, 'mode': mode,
                                          'binary': str(binary), 'sha256': binary_hash,
                                          'manifest': str(sidecar), 'manifest_sha256': sidecar_hash})
            for stress in (False, True):
                current = dict(environment)
                if stress:
                    current['DARTFORGE_GC_STRESS'] = '1'
                case = label + ('-stress' if stress else '-normal')
                actual = run([binary], case, current, timeout=45)
                if (actual.stdout, actual.stderr) != (reference.stdout, reference.stderr):
                    raise RuntimeError(f'{case}: native output differs from VM')
                if sha256(binary) != binary_hash or sha256(sidecar) != sidecar_hash:
                    raise RuntimeError(f'{case}: artifact changed during execution')
                metadata['runs'].append({'probe': source.name, 'mode': mode,
                                         'gc_stress': stress, 'matches_vm_and_official_aot': True,
                                         'stdout_bytes': len(actual.stdout),
                                         'stdout_sha256': hashlib.sha256(actual.stdout).hexdigest()})
                save()
                print(f'{case}: verified', flush=True)
    current_inputs = [config, PACKAGE / 'pubspec.yaml', *sorted((PACKAGE / 'lib').rglob('*.dart')), *sources]
    current = {str(path.relative_to(ROOT)): sha256(path) for path in current_inputs}
    if current != hashes or sha256(compiler) != metadata['compiler_sha256']:
        raise RuntimeError('Package inputs or compiler changed during validation')
    metadata['inputs_stable_through_run'] = True
    save()
    print(ledger, flush=True)


if __name__ == '__main__':
    main()

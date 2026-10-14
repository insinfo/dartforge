#!/usr/bin/env python3
"""Verify StringBuffer UTF-16, JSON and isolate semantics across memory modes.

This is a correctness matrix, not a benchmark or an ownership certificate.
All artifacts and temporary compiler files are written under the output directory.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
SOURCE_HASH = 'bd80cba83bca45bcd6116f81de36ebfd417b15a1f92d07a2d5668d828742aa27'
TRACE_HASH = '459957fa6f6f5d03b32556d545fbbca2fb731c931920486869486c6c8065c330'
TRACE_BYTES = 1012


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', type=Path, default=ROOT / 'target/p4/release/dartforge')
    parser.add_argument('--dart', type=Path, default=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart'))
    parser.add_argument('--clang', type=Path, default=Path('/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang'))
    parser.add_argument('--output', type=Path, default=ROOT / 'target/string-buffer-validation')
    parser.add_argument('--oracle-only', action='store_true', help='Run only VM and official AOT; do not invoke DartForge')
    args = parser.parse_args()
    compiler, dart, clang, output = (p.resolve() for p in (args.compiler, args.dart, args.clang, args.output))
    source = ROOT / 'bench/comparison/string_buffer.dart'
    sdk = dart.parent.parent / 'lib'
    if sha(source) != SOURCE_HASH:
        raise RuntimeError('Frozen fixture changed; review the expected semantic oracle before updating it')
    if not (sdk / 'libraries.json').is_file():
        raise RuntimeError('Dart executable must be inside its SDK')
    if (output / 'validation.json').exists():
        raise RuntimeError('Choose a new output directory to preserve earlier validation evidence')
    output.mkdir(parents=True, exist_ok=True)
    environment = dict(os.environ, TMPDIR=str(output), DARTFORGE_CLANG=str(clang),
                       DARTFORGE_EFFECTS='check', DARTFORGE_ARC_CHECK='1')
    for key in ('DARTFORGE_GC_STRESS', 'DARTFORGE_GC_SCHEDULE', 'DARTFORGE_ARC_PREPARED',
                'DARTFORGE_ARC_PREPARE', 'DARTFORGE_CONTRACTS'):
        environment.pop(key, None)
    inputs = {str(source): sha(source), str(dart): sha(dart),
              str(Path(__file__).resolve()): sha(Path(__file__).resolve())}
    if not args.oracle_only:
        inputs.update({str(compiler): sha(compiler), str(clang): sha(clang)})
    ledger = {'source': str(source), 'source_sha256': SOURCE_HASH,
              'runner_sha256': sha(Path(__file__).resolve()), 'sdk_lib': str(sdk),
              'compiler': None if args.oracle_only else str(compiler),
              'compiler_sha256': None if args.oracle_only else sha(compiler),
              'clang': str(clang), 'input_hashes': inputs, 'commands': [], 'builds': [], 'runs': [],
              'expected_stdout_sha256': TRACE_HASH, 'expected_stdout_bytes': TRACE_BYTES,
              'oracle_only': args.oracle_only, 'inputs_stable_through_run': False,
              'complete': False, 'performance_measurement': False,
              'limits': ['The fixture checks exact UTF-16 units including lone surrogates, immutable snapshots, conversion effects/errors, JSON and isolate copies.',
                         'Successful behavior is not a NoEscape, ARC ownership or complete backend-effect certificate.',
                         'Manifest identity is checked against its binary path; independent SHA256 binds artifacts. Compiler BLAKE3 is recorded without independent recomputation.']}

    def save():
        (output / 'validation.json').write_text(json.dumps(ledger, indent=2) + '\n')

    def run(command, label, current=environment, timeout=180):
        command = [str(value) for value in command]
        try:
            result = subprocess.run(command, cwd=ROOT, env=current, capture_output=True, timeout=timeout)
        except subprocess.TimeoutExpired as error:
            (output / (label + '.stdout')).write_bytes(error.stdout or b'')
            (output / (label + '.stderr')).write_bytes(error.stderr or b'')
            ledger['commands'].append({'label': label, 'command': command, 'timeout_seconds': timeout})
            save()
            raise RuntimeError(f'{label} timed out; inspect {output}') from error
        (output / (label + '.stdout')).write_bytes(result.stdout)
        (output / (label + '.stderr')).write_bytes(result.stderr)
        ledger['commands'].append({'label': label, 'command': command, 'exit_code': result.returncode,
                                   'environment': {key: value for key, value in current.items()
                                                   if key.startswith('DARTFORGE_') or key == 'TMPDIR'}})
        save()
        if result.returncode:
            raise RuntimeError(f'{label} exited {result.returncode}; inspect {output}')
        return result

    def verify(result, label):
        digest = hashlib.sha256(result.stdout).hexdigest()
        if len(result.stdout) != TRACE_BYTES or digest != TRACE_HASH or result.stderr:
            raise RuntimeError(f'{label}: UTF-16/JSON/isolate trace differs from the reviewed oracle or stderr is nonempty')
        return {'label': label, 'exit_code': 0, 'stdout_bytes': len(result.stdout),
                'stdout_sha256': digest, 'stderr_bytes': 0, 'matches_reviewed_oracle': True}

    save()
    version = run([dart, '--version'], 'sdk-version')
    ledger['sdk_version'] = (version.stdout + version.stderr).decode().strip()
    ledger['runs'].append(verify(run([dart, source], 'vm', timeout=45), 'vm'))
    official = output / 'official'
    run([dart, 'compile', 'exe', source, '-o', official], 'official-build')
    official_hash = sha(official)
    record = verify(run([official], 'official', timeout=45), 'official')
    record['binary_sha256'] = official_hash
    ledger['runs'].append(record)
    if sha(official) != official_hash:
        raise RuntimeError('Official binary changed during validation')
    save()
    if not args.oracle_only:
        for mode in ('tracing', 'arc', 'prepared'):
            current = dict(environment)
            if mode == 'prepared':
                current['DARTFORGE_ARC_PREPARED'] = '1'
            options = ['--sdk', sdk, '--optimize', '--exceptions=tables', '--roots=shadow',
                       '--stack-trace=none', '--dartforge-contracts=verify',
                       '--memory=' + ('tracing' if mode == 'tracing' else 'arc')]
            binary = output / ('string-buffer.' + mode)
            run([compiler, 'compile-native', source, *options, '-o', binary], mode + '-build', current)
            sidecar = Path(str(binary) + '.contracts.json')
            manifest = json.loads(sidecar.read_text())
            if manifest['mode'] != 'verify' or manifest.get('certified') or manifest.get('requests') or manifest.get('diagnostics'):
                raise RuntimeError(f'{mode}: unexpected contract metadata')
            if Path(manifest['artifact']['path']).resolve() != binary:
                raise RuntimeError(f'{mode}: manifest is not bound to this binary')
            binary_hash, sidecar_hash = sha(binary), sha(sidecar)
            ledger['builds'].append({'mode': mode, 'binary': str(binary), 'binary_sha256': binary_hash,
                                     'manifest_path': str(sidecar), 'manifest_sha256': sidecar_hash,
                                     'artifact_identity': manifest['artifact'], 'certified': False,
                                     'requests': manifest.get('requests', [])})
            for stress in (False, True):
                runtime = dict(current)
                if stress:
                    runtime['DARTFORGE_GC_STRESS'] = '1'
                label = mode + ('-stress' if stress else '-normal')
                record = verify(run([binary], label, runtime, timeout=45), label)
                record.update(mode=mode, gc_stress=stress, binary_sha256=binary_hash,
                              manifest_sha256=sidecar_hash)
                ledger['runs'].append(record)
                if sha(binary) != binary_hash or sha(sidecar) != sidecar_hash:
                    raise RuntimeError('Binary or sidecar changed during validation')
                save()
                print(f'{label}: exact UTF-16/JSON/isolate oracle verified', flush=True)
    for path, expected in inputs.items():
        if sha(Path(path)) != expected:
            raise RuntimeError(f'Input changed during validation: {path}')
    ledger['inputs_stable_through_run'] = True
    ledger['complete'] = True
    save()
    print(output / 'validation.json', flush=True)


if __name__ == '__main__':
    main()

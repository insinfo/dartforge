#!/usr/bin/env python3
"""Verify null predicates and eager Boolean effects across native memory modes.

Records exact VM/upstream/native traces and emitted predicate bodies, without
claiming a complete backend effect certificate or a performance improvement.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
EXPECTED = (b'true false false true equality=0\n'
            b'true false eager=2\n'
            b'false false false false equality=0\n')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', type=Path, default=ROOT / 'target/p4/release/dartforge')
    parser.add_argument('--dart', default='dart')
    parser.add_argument('--output', type=Path, default=ROOT / 'target/nullable-equality')
    args = parser.parse_args()
    compiler, output = args.compiler.resolve(), args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    source = ROOT / 'bench/comparison/nullable_equality.dart'
    sdk = Path(shutil.which(args.dart) or args.dart).resolve().parent.parent / 'lib'
    if not (sdk / 'libraries.json').is_file():
        raise RuntimeError('Dart executable must resolve inside a Dart SDK')
    environment = dict(os.environ, TMPDIR=str(output), DARTFORGE_EFFECTS='check', DARTFORGE_ARC_CHECK='1')
    for key in ('DARTFORGE_GC_STRESS', 'DARTFORGE_GC_SCHEDULE', 'DARTFORGE_ARC_PREPARED', 'DARTFORGE_ARC_PREPARE', 'DARTFORGE_CONTRACTS'):
        environment.pop(key, None)
    ledger = {'compiler': str(compiler), 'compiler_sha256': sha(compiler),
              'source': str(source), 'source_sha256': sha(source),
              'runner_sha256': sha(Path(__file__).resolve()), 'sdk_lib': str(sdk),
              'expected_trace': EXPECTED.decode(), 'commands': [], 'builds': [], 'runs': [], 'ir': [],
              'inputs_stable_through_run': False, 'performance_measurement': False,
              'limits': ['IR checks cover the named predicate definitions before Clang/LTO, not every emitted helper or a NoEscape certificate.',
                         'Manifest paths and independent SHA256 bind binaries; compiler-provided BLAKE3 is recorded without independent recomputation.']}

    def save():
        (output / 'validation.json').write_text(json.dumps(ledger, indent=2) + '\n')

    def run(command, label, current=environment, timeout=180):
        command = [str(value) for value in command]
        result = subprocess.run(command, cwd=ROOT, env=current, capture_output=True, timeout=timeout)
        (output / (label + '.stdout')).write_bytes(result.stdout)
        (output / (label + '.stderr')).write_bytes(result.stderr)
        ledger['commands'].append({'label': label, 'command': command, 'exit_code': result.returncode})
        save()
        if result.returncode:
            raise RuntimeError(f'{label} exited {result.returncode}; inspect {output}')
        return result

    def verify(result, label):
        if result.stdout != EXPECTED or result.stderr:
            raise RuntimeError(f'{label}: unexpected trace or stderr')
        return {'label': label, 'exit_code': 0, 'stdout_bytes': len(result.stdout),
                'stdout_sha256': hashlib.sha256(result.stdout).hexdigest(), 'stderr_bytes': 0,
                'matches_explicit_expected_trace': True}

    ledger['sdk_version'] = run([args.dart, '--version'], 'sdk-version').stdout.decode().strip()
    ledger['runs'].append(verify(run([args.dart, source], 'vm'), 'vm'))
    official = output / 'official'
    run([args.dart, 'compile', 'exe', source, '-o', official], 'official-build')
    ledger['runs'].append(verify(run([official], 'official'), 'official'))
    for mode in ('tracing', 'arc', 'prepared'):
        current = dict(environment)
        if mode == 'prepared':
            current['DARTFORGE_ARC_PREPARED'] = '1'
        options = ['--sdk', sdk, '--optimize', '--exceptions=tables', '--roots=shadow',
                   '--stack-trace=none', '--dartforge-contracts=verify',
                   '--memory=' + ('tracing' if mode == 'tracing' else 'arc')]
        binary = output / ('nullable.' + mode)
        run([compiler, 'compile-native', source, *options, '-o', binary], mode + '-build', current)
        sidecar = Path(str(binary) + '.contracts.json')
        manifest = json.loads(sidecar.read_text())
        if manifest['mode'] != 'verify' or manifest.get('certified') or manifest.get('requests') or manifest.get('diagnostics'):
            raise RuntimeError(f'{mode}: unexpected contract metadata')
        if Path(manifest['artifact']['path']).resolve() != binary:
            raise RuntimeError(f'{mode}: manifest not bound to this binary')
        binary_hash, sidecar_hash = sha(binary), sha(sidecar)
        ledger['builds'].append({'mode': mode, 'binary': str(binary), 'binary_sha256': binary_hash,
                                 'manifest_path': str(sidecar), 'manifest_sha256': sidecar_hash,
                                 'artifact_identity': manifest['artifact']})
        ir = output / ('nullable.' + mode + '.ll')
        run([compiler, 'compile-native', source, *options, '--emit-ir', '-o', ir], mode + '-ir', current)
        definitions = re.findall(r'^define [^\n]*@([^\s(]+)\([^\n]*\{\n(.*?)^\}', ir.read_text(), re.M | re.S)
        predicates = []
        for symbol, body in definitions:
            if not re.search(r'\.\.(missing|present)$', symbol):
                continue
            if not re.search(r'icmp (eq|ne) i64 %v\d+, 0', body):
                raise RuntimeError(f'{mode}: {symbol} lost its real parameter null predicate')
            if re.search(r'@(?:df\.selector|dartforge_selector|dartforge_.*equal)', body):
                raise RuntimeError(f'{mode}: equality dispatch survives in {symbol}')
            predicates.append({'symbol': symbol, 'body_sha256': hashlib.sha256(body.encode()).hexdigest(),
                               'parameter_null_comparison_present': True, 'equality_dispatch_absent': True})
        if len(predicates) != 2:
            raise RuntimeError(f'{mode}: expected two inspectable never-inline predicates, got {len(predicates)}')
        ledger['ir'].append({'mode': mode, 'path': str(ir), 'sha256': sha(ir), 'predicates': predicates})
        for stress in (False, True):
            runtime = dict(current)
            if stress:
                runtime['DARTFORGE_GC_STRESS'] = '1'
            label = mode + ('-stress' if stress else '-normal')
            result = verify(run([binary], label, runtime, timeout=45), label)
            result.update(mode=mode, gc_stress=stress, binary_sha256=binary_hash,
                          manifest_sha256=sidecar_hash)
            ledger['runs'].append(result)
            if sha(binary) != binary_hash or sha(sidecar) != sidecar_hash:
                raise RuntimeError('Binary or sidecar changed during validation')
            save()
            print(f'{label}: semantic oracle and predicate IR verified', flush=True)
    if sha(compiler) != ledger['compiler_sha256'] or sha(source) != ledger['source_sha256']:
        raise RuntimeError('Compiler or source changed during validation')
    ledger['inputs_stable_through_run'] = True
    save()
    print(output / 'validation.json', flush=True)


if __name__ == '__main__':
    main()

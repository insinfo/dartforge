#!/usr/bin/env python3
"""Compare ordinary Dart receiver-cone fixtures and inspect native reachability."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', type=Path, default=root / 'target/p4/release/dartforge')
    parser.add_argument('--dart', type=Path, default=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart'))
    parser.add_argument('--clang', type=Path, default=Path('/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang'))
    parser.add_argument('--output', type=Path, default=root / 'target/receiver-cone-validation')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    fixtures = root / 'crates/emit_native/tests/fixtures/receiver_cones'
    sources = sorted(fixtures.glob('*.dart'))
    ledger = {
        'schema': 1, 'compiler_sha256': digest(args.compiler),
        'sources': {str(p.relative_to(root)): digest(p) for p in sources},
        'runs': [], 'ir_observations': {}, 'performance_measurement': False,
        'effectiveness_failures': [],
        'scope': 'Exact SDK formal-this receiver facts; unknown receivers remain conservative.',
    }
    env = dict(os.environ, TMPDIR=str(args.output.resolve()),
               DARTFORGE_CLANG=str(args.clang), DARTFORGE_EFFECTS='check',
               DARTFORGE_ARC_CHECK='1')
    env['PATH'] = str(args.clang.parent) + os.pathsep + env.get('PATH', '')
    for key in ('DARTFORGE_GC_STRESS', 'DARTFORGE_GC_OFF', 'DARTFORGE_GC_SCHEDULE',
                'DARTFORGE_ARC_PREPARED', 'DARTFORGE_NO_PRUNE', 'DARTFORGE_WHY'):
        env.pop(key, None)

    def save():
        (args.output / 'ledger.json').write_text(json.dumps(ledger, indent=2) + '\n')

    def run(label, command, oracle=None, extra=None):
        command = list(map(str, command))
        current = dict(env)
        current.update(extra or {})
        result = subprocess.run(command, cwd=root, env=current, capture_output=True, timeout=600)
        (args.output / (label + '.stdout')).write_bytes(result.stdout)
        (args.output / (label + '.stderr')).write_bytes(result.stderr)
        passed = result.returncode == 0 and (oracle is None or
                                             (result.stdout == oracle and not result.stderr))
        ledger['runs'].append({'label': label, 'command': command,
                               'exit_code': result.returncode, 'passed': passed,
                               'stdout_sha256': hashlib.sha256(result.stdout).hexdigest()})
        save()
        if not passed:
            raise RuntimeError(f'{label} failed; inspect {args.output}')
        print(f'{label} PASS', flush=True)
        return result.stdout

    flags = ['--sdk', args.dart.parent.parent / 'lib', '--optimize',
             '--exceptions=tables', '--roots=shadow', '--stack-trace=none', '--memory=tracing']
    marker = 'UNREACHABLE_COLD_LENGTH_BODY'
    for source in sources:
        name = source.stem
        binary = args.output / name
        ir = args.output / (name + '.ll')
        oracle = run(name + '-vm', [args.dart, source])
        run(name + '-build', [args.compiler, 'compile-native', source, *flags, '-o', binary],
            extra={'DARTFORGE_WHY': str((args.output / (name + '.why.tsv')).resolve())})
        run(name + '-native', [binary], oracle)
        run(name + '-stress', [binary], oracle, {'DARTFORGE_GC_STRESS': '1'})
        if name in ('dynamic_length', 'unary_tearoff'):
            branch = 'cold' if name == 'dynamic_length' else 'dynamic'
            cold = run(name + '-' + branch + '-vm', [args.dart, source, branch])
            run(name + '-' + branch + '-native', [binary, branch], cold)
            run(name + '-' + branch + '-stress', [binary, branch], cold, {'DARTFORGE_GC_STRESS': '1'})
        run(name + '-ir', [args.compiler, 'compile-native', source, *flags, '--emit-ir', '-o', ir])
        text = ir.read_text()
        found = marker in text
        ledger['ir_observations'][name] = {'cold_marker_present': found,
                                          'ir_bytes': ir.stat().st_size,
                                          'binary_sha256': digest(binary)}
        save()
        if name == 'unrelated_length' and found:
            ledger['effectiveness_failures'].append('Unrelated Cold.length body remains reachable')
        if name == 'dynamic_length' and not found:
            raise AssertionError('Unknown dynamic receiver lost Cold.length body')
    ledger['inputs_stable'] = digest(args.compiler) == ledger['compiler_sha256'] and all(
        digest(root / relative) == checksum for relative, checksum in ledger['sources'].items())
    ledger['correctness_passed'] = all(run['passed'] for run in ledger['runs'])
    ledger['effectiveness_passed'] = not ledger['effectiveness_failures']
    save()
    if not ledger['inputs_stable']:
        raise AssertionError('Compiler or fixture changed during verification')
    if ledger['effectiveness_failures']:
        raise AssertionError('Correctness passed; whole-program cold-body exclusion was not achieved')
    print('Receiver-cone pruning validated', flush=True)


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Prepare frozen encoding baselines or compare two binaries with a VM oracle."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'bench/comparison/json_encode.dart'
DEFAULT_DART = Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart')
DEFAULT_CLANG = Path('/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def signatures(report):
    return [(s['scenario'], k, s[k]['encoded_units'], s[k]['checksum'])
            for s in report['scenarios'] for k in ('json', 'string_buffer')]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('phase', choices=('prepare', 'compare'))
    parser.add_argument('--compiler', type=Path, default=ROOT / 'target/p4/release/dartforge')
    parser.add_argument('--dart', type=Path, default=DEFAULT_DART)
    parser.add_argument('--clang', type=Path, default=DEFAULT_CLANG)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--baseline', type=Path)
    parser.add_argument('--prepared', type=Path,
                        help='Reuse a hash-bound prepared after binary for a measurement-only comparison')
    parser.add_argument('--pairs', type=int, default=3)
    parser.add_argument('--duration-ms', type=int, default=500)
    parser.add_argument('--before-list-tuple-cache', choices=('0', '1'))
    parser.add_argument('--after-list-tuple-cache', choices=('0', '1'))
    args = parser.parse_args()
    if not 3 <= args.pairs <= 10 or not 50 <= args.duration_ms <= 1000:
        parser.error('Expected 3..10 pairs and 50..1000 milliseconds per kernel')
    if args.phase == 'compare' and args.baseline is None:
        parser.error('compare requires --baseline containing a prepared ledger and binary')
    if args.prepared is not None and args.phase != 'compare':
        parser.error('--prepared requires compare')
    if (args.before_list_tuple_cache is None) != (args.after_list_tuple_cache is None):
        parser.error('Specify both list-tuple cache policies for an ablation')
    if args.before_list_tuple_cache is not None and args.phase != 'compare':
        parser.error('List-tuple cache ablation requires compare')
    output = args.output.resolve()
    if output.exists() and any(output.iterdir()):
        parser.error('Output must be absent or empty; preserve existing evidence')
    output.mkdir(parents=True, exist_ok=True)
    compiler, dart, clang = (p.resolve() for p in (args.compiler, args.dart, args.clang))
    env = dict(os.environ, TMPDIR=str(output), DARTFORGE_CLANG=str(clang),
               DARTFORGE_EFFECTS='check', DARTFORGE_ARC_CHECK='1')
    env['PATH'] = str(clang.parent) + os.pathsep + env.get('PATH', '')
    for key in ('DARTFORGE_GC_STRESS', 'DARTFORGE_GC_OFF', 'DARTFORGE_GC_SCHEDULE',
                'DARTFORGE_ARC_PREPARED', 'DARTFORGE_ARC_PREPARE', 'DARTFORGE_PART_MB'):
        env.pop(key, None)
    env.pop('DARTFORGE_RTI_LIST_TUPLE_CACHE', None)
    hashes = {str(p): sha(p) for p in (SOURCE, compiler, dart, clang)}
    ledger = {'schema': 1, 'phase': args.phase, 'input_hashes': hashes,
              'commands': [], 'measurements': [], 'http_performance_claim': False,
              'limitations': ['Isolated JSON/StringBuffer kernels, not an HTTP workload.',
                              'Fresh processes reuse their input graph within each kernel.',
                              'Short samples do not establish production throughput.'],
              'compile_environment': {k: env[k] for k in ('TMPDIR', 'DARTFORGE_CLANG',
                                                        'DARTFORGE_EFFECTS', 'DARTFORGE_ARC_CHECK')}}

    def save():
        (output / 'ledger.json').write_text(json.dumps(ledger, indent=2) + '\n')

    def run(label, command, oracle=False, overrides=None):
        command = list(map(str, command))
        result = subprocess.run(command, cwd=ROOT, env=dict(env, **(overrides or {})), capture_output=True, timeout=600)
        (output / (label + '.stdout')).write_bytes(result.stdout)
        (output / (label + '.stderr')).write_bytes(result.stderr)
        ledger['commands'].append({'label': label, 'command': command,
                                   'exit_code': result.returncode})
        if overrides:
            ledger['commands'][-1]['environment_overrides'] = overrides
        save()
        if result.returncode or (oracle and result.stderr):
            raise RuntimeError(f'{label} failed; inspect {output}')
        print(label + ' PASS', flush=True)
        return json.loads(result.stdout) if oracle else None

    run('dart-version', [dart, '--version'])
    run('clang-version', [clang, '--version'])
    oracle = run('vm-validation', [dart, SOURCE, '--validate'], oracle=True)
    ledger['oracle'] = oracle
    binary = output / 'native'
    if args.prepared is None:
        run('native-build', [compiler, 'compile-native', SOURCE, '--sdk', dart.parent.parent / 'lib',
                             '--optimize', '--exceptions=tables', '--roots=shadow',
                             '--stack-trace=none', '--memory=tracing', '--timings', '-o', binary])
    else:
        prepared = args.prepared.resolve()
        prepared_ledger = json.loads((prepared / 'ledger.json').read_text())
        prepared_binary = prepared / 'native'
        if (prepared_ledger['phase'] != 'prepare' or prepared_ledger['oracle'] != oracle
                or prepared_ledger['input_hashes'] != hashes
                or sha(prepared_binary) != prepared_ledger['binary_sha256']):
            raise RuntimeError('Prepared after binary inputs/oracle/hash mismatch')
        shutil.copy2(prepared_binary, binary)
        ledger['prepared_after_ledger'] = str(prepared / 'ledger.json')
    native = run('native-validation', [binary, '--validate'], oracle=True)
    if native != oracle:
        raise RuntimeError('Native validation differs from the official VM')
    ledger['binary_sha256'] = sha(binary)
    if args.phase == 'compare':
        baseline = args.baseline.resolve()
        prior = json.loads((baseline / 'ledger.json').read_text())
        old = baseline / 'native'
        if prior['phase'] != 'prepare' or prior['oracle'] != oracle:
            raise RuntimeError('Baseline preparation/oracle mismatch')
        if prior['input_hashes'].get(str(SOURCE)) != hashes[str(SOURCE)]:
            raise RuntimeError('Benchmark source changed since baseline compilation')
        if sha(old) != prior['binary_sha256']:
            raise RuntimeError('Baseline binary hash changed')
        ledger['baseline_ledger'] = str(baseline / 'ledger.json')
        ledger['baseline_binary_sha256'] = sha(old)
        for pair in range(args.pairs):
            order = [('before', old), ('after', binary)]
            if pair % 2:
                order.reverse()
            for variant, executable in order:
                expected_hash = (prior['binary_sha256'] if variant == 'before'
                                 else ledger['binary_sha256'])
                if sha(executable) != expected_hash:
                    raise RuntimeError('Measured binary changed')
                report = run(f'pair-{pair + 1}-{variant}',
                             [executable, f'--duration-ms={args.duration_ms}'], oracle=True,
                             overrides=({'DARTFORGE_RTI_LIST_TUPLE_CACHE':
                                         args.before_list_tuple_cache if variant == 'before'
                                         else args.after_list_tuple_cache}
                                        if args.before_list_tuple_cache is not None else None))
                if signatures(report) != signatures(oracle):
                    raise RuntimeError('Measured content differs from VM oracle')
                if sha(executable) != expected_hash:
                    raise RuntimeError('Measured binary changed during execution')
                ledger['measurements'].append({'pair': pair + 1, 'variant': variant,
                                               'report': report})
                save()
    ledger['inputs_stable'] = all(sha(Path(p)) == digest for p, digest in hashes.items())
    if not ledger['inputs_stable']:
        raise RuntimeError('Inputs changed during validation')
    save()


if __name__ == '__main__':
    main()

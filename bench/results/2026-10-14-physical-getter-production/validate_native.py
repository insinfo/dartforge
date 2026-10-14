"""Validate certified tracing getters against the VM with the pass enabled and disabled."""
from pathlib import Path
import argparse
import gzip
import hashlib
import json
import os
import re
import shutil
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--compiler', required=True, type=Path)
parser.add_argument('--compiler-sha256', required=True)
args = parser.parse_args()
root = Path.cwd()
out = root / 'bench/results/2026-10-14-physical-getter-production'
work = root / 'target/native-provenance/physical-getter-production'
work.mkdir(parents=True, exist_ok=True)
compiler = args.compiler.resolve()
sdk = Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk')
fixtures = {name: root / 'crates/emit_native/tests/fixtures/devirtualization' / (name + '.dart')
            for name in ['physical_getters', 'physical_getter_boundaries']}


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


assert sha(compiler) == args.compiler_sha256
inputs = {str(path): sha(path) for path in fixtures.values()}
ledger = {'compiler': str(compiler), 'compiler_sha256': sha(compiler), 'inputs': inputs,
          'scope': 'Tracing-only correctness and emitted-IR evidence. No performance or size improvement claim; ARC remains excluded by the production guard.',
          'runs': [], 'ir': {}, 'complete': False}
for name in ['provenance.json', 'source.json']:
    source = compiler.parent / name
    if source.exists():
        shutil.copy2(source, out / ('native-compiler-' + name))
env = {key: value for key, value in os.environ.items() if not key.startswith('DARTFORGE_')}
env.update(DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang',
           DARTFORGE_EFFECTS='check', DARTFORGE_ARC_CHECK='1', DARTFORGE_KEEP_IR='1',
           DARTFORGE_ARC_PREPARED='0', DARTFORGE_ARC_PORT_PRUNING='1',
           DARTFORGE_FACTORY_RECEIVER_PROVENANCE='1')


def save():
    (out / 'native-validation.json').write_text(json.dumps(ledger, indent=2) + '\n')


def run(label, command, execution):
    command = list(map(str, command))
    print(label + ' starting', flush=True)
    result = subprocess.run(command, cwd=root, env=execution, capture_output=True, timeout=1800)
    (out / (label + '.stdout')).write_bytes(result.stdout)
    (out / (label + '.stderr')).write_bytes(result.stderr)
    row = {'label': label, 'command': command, 'exit': result.returncode,
           'environment': {key: value for key, value in execution.items()
                           if key.startswith('DARTFORGE_') or key in ['TMPDIR', 'PUB_CACHE']},
           'stdout_sha256': hashlib.sha256(result.stdout).hexdigest(),
           'stderr_sha256': hashlib.sha256(result.stderr).hexdigest()}
    ledger['runs'].append(row)
    save()
    print(label + ' terminal ' + str(result.returncode), flush=True)
    return result, row


version, _ = run('sdk-version', [sdk / 'bin/dart', '--version'], env)
assert version.returncode == 0
for name, fixture in fixtures.items():
    oracle, _ = run(name + '-vm', [sdk / 'bin/dart', fixture], env)
    assert oracle.returncode == 0 and not oracle.stderr
    ledger['ir'][name] = {}
    for disabled in [False, True]:
        mode = 'off' if disabled else 'on'
        label = name + '-' + mode
        folder = work / label
        folder.mkdir(exist_ok=True)
        binary = folder / 'program'
        execution = dict(env, DARTFORGE_NO_PHYSICAL_GETTER_DEVIRTUALIZATION='1' if disabled else '0')
        result, row = run(label + '-build', [compiler, 'compile-native', fixture, '--sdk', sdk / 'lib',
                                          '--optimize', '--memory=tracing', '--exceptions=check',
                                          '--roots=shadow', '--stack-trace=none', '-o', binary], execution)
        if result.returncode != 0:
            continue
        row.update(binary=str(binary), binary_bytes=binary.stat().st_size, binary_sha256=sha(binary))
        ir_path = folder / '.df_tmp/program.ll'
        ir = ir_path.read_text()
        with gzip.open(out / (label + '.ll.gz'), 'wb') as archive:
            archive.write(ir.encode())
        ledger['ir'][name][mode] = {
            'path': str(ir_path), 'sha256': sha(ir_path),
            'selector_lookup_calls': len(re.findall(r'call ptr @df\.selector(?:_d)?\(', ir)),
            'direct_getter_wrapper_calls': len(re.findall(r'call i64 @[^\s(]*(?:Holder|Base)\.(?:value|failure)\$g\(', ir)),
        }
        save()
        for stress in [False, True]:
            run_env = execution.copy()
            if stress:
                run_env['DARTFORGE_GC_STRESS'] = '1'
            result, row = run(label + ('-stress' if stress else '-normal'), [binary], run_env)
            row['vm_exact'] = result.returncode == 0 and not result.stderr and result.stdout == oracle.stdout
            save()
            print(row['label'] + ' exact VM ' + str(row['vm_exact']), flush=True)
ledger['compiler_unchanged'] = sha(compiler) == args.compiler_sha256
ledger['inputs_unchanged'] = all(sha(path) == digest for path, digest in inputs.items())
ledger['eight_native_runs_exact_vm'] = sum(row.get('vm_exact', False) for row in ledger['runs']) == 8
ledger['ir_proves_direct_calls'] = all(
    set(modes) == {'on', 'off'} and
    modes['on']['direct_getter_wrapper_calls'] > modes['off']['direct_getter_wrapper_calls'] and
    modes['on']['selector_lookup_calls'] < modes['off']['selector_lookup_calls']
    for modes in ledger['ir'].values())
ledger['complete'] = True
save()
assert ledger['eight_native_runs_exact_vm'] and ledger['compiler_unchanged'] and ledger['inputs_unchanged'] and ledger['ir_proves_direct_calls']

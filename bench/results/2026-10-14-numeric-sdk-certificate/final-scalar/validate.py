"""Validate scalar coalesce with a frozen producer; collect IR, never performance timings."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--compiler', type=Path, required=True)
parser.add_argument('--compiler-sha256', required=True)
args = parser.parse_args()
root = Path.cwd()
out = root / 'bench/results/2026-10-14-numeric-sdk-certificate/final-scalar'
work = root / 'target/numeric-sdk-certificate/final-scalar'
work.mkdir(parents=True, exist_ok=True)
compiler = args.compiler.resolve()
sdk = Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk')
fixture = root / 'crates/emit_native/tests/fixtures/devirtualization/scalar_nullable_count.dart'

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

assert sha(compiler) == args.compiler_sha256
ledger = {'compiler': str(compiler), 'compiler_sha256': sha(compiler),
          'fixture': str(fixture), 'fixture_sha256': sha(fixture),
          'scope': 'Tracing-only scalar ON/OFF correctness, normal and GC stress; SDK VM and native AOT oracles. Checked-double pass remains enabled in both profiles. No performance measurement.',
          'runs': [], 'ir': {}, 'complete': False}
for name in ['provenance.json', 'source.json']:
    source = compiler.parent / name
    if source.exists():
        shutil.copy2(source, out / ('compiler-' + name))
        ledger[name + '_sha256'] = sha(source)
env = {k: v for k,v in os.environ.items() if not k.startswith('DARTFORGE_')}
env.update(TMPDIR=str(work),
           DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang',
           LLVM_SYS_221_PREFIX='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64',
           DARTFORGE_SDK_LIB=str(sdk / 'lib'), DARTFORGE_EFFECTS='check',
           DARTFORGE_ARC_CHECK='1', DARTFORGE_KEEP_IR='1', DARTFORGE_ARC_PREPARED='0',
           DARTFORGE_ARC_PORT_PRUNING='1', DARTFORGE_FACTORY_RECEIVER_PROVENANCE='1')

def save():
    (out / 'validation.json').write_text(json.dumps(ledger, indent=2) + '\n')

def run(label, command, execution):
    command = list(map(str, command))
    print(label + ' starting', flush=True)
    result = subprocess.run(command, cwd=root, env=execution, capture_output=True, timeout=1800)
    (out / (label + '.stdout')).write_bytes(result.stdout)
    (out / (label + '.stderr')).write_bytes(result.stderr)
    row = {'label': label, 'command': command, 'exit': result.returncode,
           'environment': {k:v for k,v in execution.items() if k.startswith('DARTFORGE_') or k in ['TMPDIR', 'LLVM_SYS_221_PREFIX', 'PUB_CACHE']},
           'stdout_sha256': hashlib.sha256(result.stdout).hexdigest(),
           'stderr_sha256': hashlib.sha256(result.stderr).hexdigest()}
    ledger['runs'].append(row)
    save()
    print(label + ' terminal ' + str(result.returncode), flush=True)
    return result, row

version, _ = run('sdk-version', [sdk / 'bin/dart', '--version'], env)
assert version.returncode == 0
oracle, _ = run('vm', [sdk / 'bin/dart', fixture], env)
assert oracle.returncode == 0 and not oracle.stderr
built, _ = run('sdk-aot-build', [sdk / 'bin/dart', 'compile', 'exe', fixture, '-o', work / 'sdk-program'], env)
assert built.returncode == 0
result, row = run('sdk-aot', [work / 'sdk-program'], env)
row['vm_exact'] = result.returncode == 0 and not result.stderr and result.stdout == oracle.stdout
save()
assert row['vm_exact']
for mode in ['off', 'on']:
    folder = work / mode
    folder.mkdir(exist_ok=True)
    binary = folder / 'program'
    execution = dict(env, DARTFORGE_NO_SCALAR_COALESCE='1' if mode == 'off' else '0')
    result, row = run(mode + '-build', [compiler, 'compile-native', fixture,
                '--sdk', sdk / 'lib', '--optimize', '--memory=tracing', '--exceptions=check',
                '--roots=shadow', '--stack-trace=none', '-o', binary], execution)
    if result.returncode != 0:
        continue
    row.update(binary=str(binary), binary_bytes=binary.stat().st_size, binary_sha256=sha(binary))
    path = folder / '.df_tmp/program.ll'
    ir = path.read_text()
    bodies = re.findall(r'^define [^\n]*@[^\s(]*\.Node\.count\([^\n]*\{\n.*?^\}', ir, re.M | re.S)
    assert len(bodies) == 1, 'Expected exact ordinary typed Node.count body'
    body = bodies[0]
    (out / (mode + '-node-count.ll')).write_text(body + '\n')
    with gzip.open(out / (mode + '-fixture.ll.gz'), 'wb') as stream:
        stream.write(ir.encode())
    ledger['ir'][mode] = {'path': str(path), 'sha256': sha(path),
        'integer_boxes': len(re.findall(r'call i64 @df\.box_int\(', body)),
        'integer_unboxes': len(re.findall(r'call i64 @df\.unbox_int\(', body)),
        'recursive_calls': len(re.findall(r'call i64 @[^\s(]*\.Node\.count\(', body)),
        'pending_checks': len(re.findall(r'(?:call i8 @dartforge_exception_pending\(|load i8, ptr %ctx, align 8)', body)),
        'pending_check_instructions': re.findall(r'^\s+[^\n]*(?:call i8 @dartforge_exception_pending\(|load i8, ptr %ctx, align 8)[^\n]*', body, re.M),
        'branch_instructions': re.findall(r'^\s+br [^\n]+', body, re.M)}
    save()
    for stress in [False, True]:
        runtime = execution.copy()
        if stress:
            runtime['DARTFORGE_GC_STRESS'] = '1'
        result, row = run(mode + ('-stress' if stress else '-normal'), [binary], runtime)
        row['vm_exact'] = result.returncode == 0 and not result.stderr and result.stdout == oracle.stdout
        save()
        print(row['label'] + ' exact VM ' + str(row['vm_exact']), flush=True)
ledger['compiler_unchanged'] = sha(compiler) == args.compiler_sha256
ledger['fixture_unchanged'] = sha(fixture) == ledger['fixture_sha256']
ledger['four_native_runs_exact_vm'] = sum(r.get('vm_exact', False) for r in ledger['runs'] if r['label'].startswith(('off-', 'on-'))) == 4
if set(ledger['ir']) == {'on', 'off'}:
    before, after = ledger['ir']['off'], ledger['ir']['on']
    ledger['ir_proof'] = before['integer_boxes'] == 2 and before['integer_unboxes'] == 2 and after['integer_boxes'] == 0 and after['integer_unboxes'] == 0 and before['recursive_calls'] == after['recursive_calls'] == 2 and before['pending_checks'] == after['pending_checks'] == 4 and before['pending_check_instructions'] == after['pending_check_instructions'] and before['branch_instructions'] == after['branch_instructions']
else:
    ledger['ir_proof'] = False
ledger['complete'] = True
save()
assert ledger['four_native_runs_exact_vm'] and ledger['compiler_unchanged'] and ledger['fixture_unchanged'] and ledger['ir_proof']

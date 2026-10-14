"""Compare the bounded candidate-lifetime probe with Dart VM and AOT."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import shutil
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--compiler', required=True, type=Path)
parser.add_argument('--compiler-sha256', required=True)
args = parser.parse_args()
root = Path.cwd()
out = root / 'bench/results/2026-10-14-arc-candidate-queue'
work = root / 'target/native-provenance/arc-candidate-correctness'
fixture = work / 'candidate_lifetimes.dart'
compiler = args.compiler.resolve()
sdk = Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk')


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


assert sha(compiler) == args.compiler_sha256
fixture_hash = sha(fixture)
shutil.copy2(fixture, out / 'candidate_lifetimes.dart')
for name in ['provenance.json', 'source.json']:
    path = compiler.parent / name
    if path.exists():
        shutil.copy2(path, out / ('native-compiler-' + name))
ledger = {
    'compiler': str(compiler), 'compiler_sha256': sha(compiler),
    'fixture': str(fixture), 'fixture_sha256': fixture_hash,
    'runtime_source_sha256': {str(path): sha(path) for path in [
        root / 'crates/runtime/src/arc.rs', root / 'crates/runtime/src/heap.rs',
        root / 'crates/runtime/src/arc/candidates.rs']},
    'scope': 'Correctness only; no timings. Candidate retain/prune execution modes share the same 32-byte metadata and binaries.',
    'runs': [], 'complete': False,
}
env = os.environ.copy()
for key in list(env):
    if key.startswith('DARTFORGE_'):
        del env[key]
env.update(DARTFORGE_ARC_CHECK='1', DARTFORGE_EFFECTS='check',
           DARTFORGE_ARC_PORT_PRUNING='1', DARTFORGE_FACTORY_RECEIVER_PROVENANCE='1',
           DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang',
           DARTFORGE_TEST_SDK_LIB=str(sdk / 'lib'), DARTFORGE_SDK_LIB=str(sdk / 'lib'))


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
oracle, _ = run('fixture-vm', [sdk / 'bin/dart', fixture], env)
assert oracle.returncode == 0 and not oracle.stderr
result, _ = run('fixture-aot-build', [sdk / 'bin/dart', 'compile', 'exe', fixture, '-o', work / 'oracle'], env)
assert result.returncode == 0
result, row = run('fixture-aot', [work / 'oracle'], env)
row['vm_exact'] = result.returncode == 0 and not result.stderr and result.stdout == oracle.stdout
save()
assert row['vm_exact']
for mode in ['legacy-arc', 'prepared-arc']:
    execution = dict(env, DARTFORGE_ARC_PREPARED='1' if mode == 'prepared-arc' else '0')
    folder = work / mode
    folder.mkdir(exist_ok=True)
    binary = folder / 'program'
    result, row = run(mode + '-build', [compiler, 'compile-native', fixture, '--sdk', sdk / 'lib',
                                      '--optimize', '--memory=arc', '--exceptions=check',
                                      '--roots=shadow', '--stack-trace=none', '-o', binary], execution)
    if result.returncode != 0:
        continue
    row.update(binary=str(binary), binary_bytes=binary.stat().st_size, binary_sha256=sha(binary))
    save()
    for retain in [False, True]:
        for stress in [False, True]:
            label = mode + ('-retain' if retain else '-prune') + ('-stress' if stress else '-normal')
            run_env = dict(execution, DARTFORGE_ARC_RETAIN_DEAD_CANDIDATES='1' if retain else '0')
            if stress:
                run_env['DARTFORGE_GC_STRESS'] = '1'
            result, row = run(label, [binary], run_env)
            row['vm_exact'] = result.returncode == 0 and not result.stderr and result.stdout == oracle.stdout
            save()
            print(label + ' exact VM ' + str(row['vm_exact']), flush=True)
ledger['compiler_unchanged'] = sha(compiler) == args.compiler_sha256
ledger['fixture_unchanged'] = sha(fixture) == fixture_hash
ledger['eight_native_runs_exact_vm'] = sum(row.get('vm_exact', False) for row in ledger['runs']
                                          if row['label'].startswith(('legacy-arc-', 'prepared-arc-'))) == 8
ledger['runtime_sources_unchanged'] = all(sha(path) == value for path, value in ledger['runtime_source_sha256'].items())
ledger['complete'] = True
save()
assert ledger['eight_native_runs_exact_vm'] and ledger['compiler_unchanged'] and ledger['fixture_unchanged'] and ledger['runtime_sources_unchanged']

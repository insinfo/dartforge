"""Compare consumed outputs of one frozen producer with primitive clamp OFF/ON."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--compiler', type=Path, required=True)
parser.add_argument('--sdk', type=Path, required=True, help='Dart SDK root (contains bin/ and lib/)')
parser.add_argument('--llvm', type=Path, required=True, help='LLVM installation root')
parser.add_argument('--packages', type=Path, required=True)
parser.add_argument('--out', type=Path, required=True)
args = parser.parse_args()
root = Path.cwd()
work = args.out.resolve()
archive = work / 'evidence'
archive.mkdir(parents=True, exist_ok=True)
work.mkdir(parents=True, exist_ok=True)
compiler = args.compiler.resolve()
sdk = args.sdk.resolve()
packages = args.packages.resolve()
llvm = args.llvm.resolve()

def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

producer_hash = digest(compiler)
environment = {key: value for key, value in os.environ.items()
               if not key.startswith('DARTFORGE_')}
environment.update(TMPDIR=str(work),
    DARTFORGE_CLANG=str(llvm / 'bin/clang'), DARTFORGE_LLVM_DIR=str(llvm),
    DARTFORGE_SDK_LIB=str(sdk / 'lib'), DARTFORGE_KEEP_IR='1', DARTFORGE_LINK_MAP='1')
cases = [('clamp', root / 'crates/emit_native/tests/fixtures/primitive_clamp.dart', 373),
         ('color', root / 'bench/comparison/color_numeric.dart', 2736)]
inputs = {str(path): digest(path) for path in
          [compiler, sdk / 'bin/dart', packages, *[source for _, source, _ in cases]]}
report = {'producer_sha256': producer_hash, 'inputs': inputs, 'cases': {}, 'scope':
          'One frozen producer, tracing/check/shadow; primitive clamp OFF/ON only. '
          'Consumed output and GC stress comparison, no performance claim.'}

def save():
    (archive / 'validation.json').write_text(json.dumps(report, indent=2) + '\n')

def run(command, folder, label, extra=None):
    folder.mkdir(parents=True, exist_ok=True)
    env = dict(environment, **(extra or {}))
    result = subprocess.run(command, env=env, capture_output=True, timeout=900)
    (folder / (label + '.stdout')).write_bytes(result.stdout)
    (folder / (label + '.stderr')).write_bytes(result.stderr)
    return {'command': command, 'exit': result.returncode,
            'stdout_sha256': digest(folder / (label + '.stdout')),
            'stderr_sha256': digest(folder / (label + '.stderr'))}

save()
for name, source, lines in cases:
    folder = archive / name
    target = work / name
    target.mkdir(exist_ok=True)
    row = {'profiles': {}}
    report['cases'][name] = row
    row['vm'] = run([str(sdk / 'bin/dart'), '--packages=' + str(packages), str(source)], folder, 'vm')
    save()
    assert row['vm']['exit'] == 0
    oracle = (folder / 'vm.stdout').read_bytes()
    assert len(oracle.splitlines()) == lines
    row['official_compile'] = run([str(sdk / 'bin/dart'), 'compile', 'exe',
        '--packages=' + str(packages), str(source), '-o', str(target / 'official')], folder, 'official-compile')
    save()
    assert row['official_compile']['exit'] == 0
    row['official'] = run([str(target / 'official')], folder, 'official')
    row['official']['exact'] = (folder / 'official.stdout').read_bytes() == oracle
    save()
    assert row['official']['exit'] == 0 and row['official']['exact']
    for profile in ['off', 'on']:
        output = target / profile
        output.mkdir(exist_ok=True)
        binary = output / 'program'
        extra = {'DARTFORGE_PRIMITIVE_CLAMP': '1' if profile == 'on' else '0'}
        data = {'compile': run([str(compiler), 'compile-native', str(source), '--sdk', str(sdk / 'lib'),
            '--packages', str(packages), '--optimize', '--memory=tracing', '--exceptions=check',
            '--roots=shadow', '--stack-trace=none', '-o', str(binary)], folder / profile, 'compile', extra)}
        row['profiles'][profile] = data
        save()
        assert data['compile']['exit'] == 0
        data['artifact'] = {'bytes': binary.stat().st_size, 'sha256': digest(binary)}
        for stress in [False, True]:
            label = 'stress' if stress else 'normal'
            runtime = dict(extra, **({'DARTFORGE_GC_STRESS': '1'} if stress else {}))
            data[label] = run([str(binary)], folder / profile, label, runtime)
            data[label]['exact'] = (folder / profile / (label + '.stdout')).read_bytes() == oracle
            save()
            assert data[label]['exit'] == 0 and data[label]['exact'], (name, profile, label)
        data['ir'] = {str(path.relative_to(output)): {'bytes': path.stat().st_size, 'sha256': digest(path)}
                      for path in output.rglob('*.ll')}
        if name == 'clamp':
            import re
            ir = (output / '.df_tmp/program.ll').read_text()
            functions = re.findall(r'^define [^\n]*\{\n.*?^\}\n', ir, re.M | re.S)
            counts = {}
            for function_name in ['unitClamp', 'typedUnitClamp', 'genericClamp']:
                matches = [body for body in functions if f'.{function_name}(' in body.splitlines()[0]]
                assert len(matches) == 1, (profile, function_name)
                counts[function_name] = len(re.findall(r'icmp sgt i64 [^\n]*, 9218868437227405312', matches[0]))
                assert (counts[function_name] > 0) == (profile == 'on' and function_name != 'genericClamp'), counts
            data['primitive_nan_guards'] = counts
        save()
        print(name, profile, 'exact', flush=True)
report['inputs_unchanged'] = all(digest(path) == value for path, value in inputs.items())
save()
assert report['inputs_unchanged']

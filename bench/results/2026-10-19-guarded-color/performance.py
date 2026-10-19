"""Paired native timings of the unchanged ordinary color kernel; correctness is separate."""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import statistics
import subprocess

p = argparse.ArgumentParser(description=__doc__)
for name in ['compiler', 'sdk', 'llvm', 'packages', 'out']:
    p.add_argument('--' + name, type=Path, required=True)
p.add_argument('--iterations', type=int, default=1000000)
a = p.parse_args()
compiler, sdk, llvm, packages, out = [v.resolve() for v in [a.compiler, a.sdk, a.llvm, a.packages, a.out]]
out.mkdir(parents=True, exist_ok=True)
source = Path.cwd() / 'bench/comparison/color_numeric_performance.dart'
digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
inputs = {str(path): digest(path) for path in [compiler, sdk / 'bin/dart', packages, source]}
env = {k: v for k, v in os.environ.items() if not k.startswith('DARTFORGE_')}
env.update(DARTFORGE_LLVM_DIR=str(llvm), DARTFORGE_CLANG=str(llvm / 'bin/clang'), DARTFORGE_KEEP_IR='1')
report = {'inputs': inputs, 'builds': [], 'runs': [], 'iterations': a.iterations,
          'scope': 'One producer; constant specialization OFF/ON only; official AOT is contextual. Three rotated process orders; first in-process sample excluded.'}
def save():
    (out / 'performance.json').write_text(json.dumps(report, indent=2) + '\n')
def run(label, command, extra=None):
    result = subprocess.run(command, env=dict(env, **(extra or {})), capture_output=True, timeout=900)
    (out / (label + '.stdout')).write_bytes(result.stdout)
    (out / (label + '.stderr')).write_bytes(result.stderr)
    assert result.returncode == 0, (label, result.stderr.decode(errors='replace')[-2000:])
    return result
binaries = {}
for policy in ['off', 'on', 'official']:
    folder = out / policy
    folder.mkdir(exist_ok=True)
    binary = folder / 'program'
    binaries[policy] = binary
    extra = {'DARTFORGE_CONSTANT_SPECIALIZATION': '1' if policy == 'on' else '0'}
    command = ([str(sdk / 'bin/dart'), 'compile', 'exe', '--packages=' + str(packages), str(source), '-o', str(binary)]
        if policy == 'official' else [str(compiler), 'compile-native', str(source), '--sdk', str(sdk / 'lib'), '--packages', str(packages),
            '--optimize', '--memory=tracing', '--exceptions=check', '--roots=shadow', '--stack-trace=none', '-o', str(binary)])
    run(policy + '-build', command, extra)
    record = {'policy': policy, 'command': command, 'exit': 0, 'sha256': digest(binary), 'bytes': binary.stat().st_size}
    if policy != 'official':
        ir = (folder / '.df_tmp/program.ll').read_text()
        bodies = re.findall(r'^define [^\n]*\{\n.*?^\}\n', ir, re.M | re.S)
        guarded = [body for body in bodies if '@df.specialized.' in body.splitlines()[0]
                   and re.search(r'call i64 @[^\s(]*convertFormatValue\(', body)]
        caller = [body for body in bodies if '.convertPixels(' in body.splitlines()[0]]
        record['guarded_color_clones'] = len(guarded)
        record['caller_uses_specialized'] = any('call i64 @df.specialized.' in body for body in caller)
        assert (len(guarded) > 0) == (policy == 'on')
        assert record['caller_uses_specialized'] == (policy == 'on')
        if guarded: (out / 'on-guarded-clone.ll').write_text(guarded[0])
    report['builds'].append(record)
    save()
checksum = None
for round_index, order in enumerate([['off', 'on', 'official'], ['on', 'official', 'off'], ['official', 'off', 'on']]):
    for policy in order:
        result = run(f'{round_index}-{policy}', [str(binaries[policy]), str(a.iterations)])
        rows = [tuple(map(int, line.split(':'))) for line in result.stdout.decode().splitlines()]
        assert len(rows) == 5
        if checksum is None: checksum = rows[0][0]
        assert all(value == checksum and elapsed > 0 for value, elapsed in rows), (policy, rows)
        report['runs'].append({'round': round_index, 'policy': policy, 'exit': 0, 'checksum': checksum,
                              'microseconds': [t for _, t in rows], 'warm_median_us': statistics.median([t for _, t in rows[1:]])})
        save()
report['median_process_warm_us'] = {policy: statistics.median([r['warm_median_us'] for r in report['runs'] if r['policy'] == policy])
                                   for policy in binaries}
report['inputs_unchanged'] = all(digest(Path(path)) == expected for path, expected in inputs.items())
assert report['inputs_unchanged']
save()
print(json.dumps(report['median_process_warm_us']))

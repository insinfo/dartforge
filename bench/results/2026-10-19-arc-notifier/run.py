"""Compare late generic notifier dispatch in tracing/ARC with the official VM."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
for name in ['compiler', 'sdk', 'llvm', 'out']:
    parser.add_argument('--' + name, type=Path, required=True)
args = parser.parse_args()
root = Path.cwd()
source = root / 'crates/emit_native/tests/fixtures/arc_ports/late_notifier_dispatch.dart'
compiler, sdk, llvm, out = [p.resolve() for p in [args.compiler, args.sdk, args.llvm, args.out]]
out.mkdir(parents=True, exist_ok=True)
digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
inputs = {str(p): digest(p) for p in [source, compiler, sdk / 'bin/dart']}
env = {k: v for k, v in os.environ.items() if not k.startswith('DARTFORGE_')}
env.update(DARTFORGE_LLVM_DIR=str(llvm), DARTFORGE_CLANG=str(llvm / 'bin/clang'))
report = {'inputs': inputs, 'runs': [], 'scope': 'macOS ARM64 functional output and checked ARC run; no performance claim'}
def run(label, command, extra=None):
    result = subprocess.run(command, env=dict(env, **(extra or {})), capture_output=True, timeout=900)
    for name, data in [('stdout', result.stdout), ('stderr', result.stderr)]:
        (out / (label + '.' + name)).write_bytes(data)
    report['runs'].append({'name': label, 'command': command, 'exit': result.returncode,
        'environment': extra or {}, 'stdout_sha256': hashlib.sha256(result.stdout).hexdigest(),
        'stderr_sha256': hashlib.sha256(result.stderr).hexdigest()})
    (out / 'validation.json').write_text(json.dumps(report, indent=2) + '\n')
    assert result.returncode == 0, (label, result.stderr.decode(errors='replace')[-2000:])
    return result.stdout
oracle = run('vm', [str(sdk / 'bin/dart'), str(source)])
for policy in ['tracing', 'arc']:
    binary = out / ('notifier-' + policy)
    run(policy + '-build', [str(compiler), 'compile-native', str(source), '--sdk', str(sdk / 'lib'),
        '--optimize', '--memory=' + policy, '--exceptions=check', '--roots=shadow', '--stack-trace=none', '-o', str(binary)])
    for stress in [False, True]:
        extra = {'DARTFORGE_ARC_CHECK': '1', 'DARTFORGE_ARC_NURSERY': '0'} if policy == 'arc' else {}
        if stress: extra['DARTFORGE_GC_STRESS'] = '1'
        actual = run(policy + ('-stress' if stress else '-normal'), [str(binary)], extra)
        assert actual == oracle, policy
        report['runs'][-1]['exact'] = True
    print(policy + ': exact', flush=True)
report['inputs_unchanged'] = all(digest(Path(p)) == value for p, value in inputs.items())
assert report['inputs_unchanged']
(out / 'validation.json').write_text(json.dumps(report, indent=2) + '\n')

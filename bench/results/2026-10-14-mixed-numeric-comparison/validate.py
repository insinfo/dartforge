"""Exercise upstream mixed comparison contracts through ordinary Dart source."""
from pathlib import Path
import hashlib, json, os, subprocess
ROOT = Path.cwd()
ARCHIVE = ROOT / 'bench/results/2026-10-14-mixed-numeric-comparison'
OUT = ROOT / 'target/mixed-numeric-comparison'
SDK = Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk')
CLI = ROOT / 'target/selector-getter-producer/dartforge'
SOURCE = ROOT / 'crates/emit_native/tests/fixtures/devirtualization/mixed_numeric_comparison.dart'
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(CLI) == '0a876b5adb34707a38fc75d4a5f74da90d22fb26fc77cf9520c403b7d5e79d2f'
OUT.mkdir(parents=True, exist_ok=True)
env = {k: v for k, v in os.environ.items() if not k.startswith('DARTFORGE_')}
env.update(TMPDIR=str(OUT), DARTFORGE_KEEP_IR='1',
           DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang',
           LLVM_SYS_221_PREFIX='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64')
report = {'source_sha256': sha(SOURCE), 'compiler_sha256': sha(CLI),
          'SDK_sha256': sha(SDK / 'bin/dart'), 'runs': {},
          'scope': 'Correctness only. Equality and relational operators round mixed operands; compareTo has a distinct exact ordering. No runtime implementation change or performance claim.'}
def run(name, command, environment=env):
    result = subprocess.run(command, env=environment, capture_output=True, text=True, timeout=600)
    (ARCHIVE / (name + '.stdout')).write_text(result.stdout)
    (ARCHIVE / (name + '.stderr')).write_text(result.stderr)
    report['runs'][name] = {'command': command, 'exit': result.returncode,
                            'stdout': result.stdout, 'stderr': result.stderr}
    (ARCHIVE / 'validation.json').write_text(json.dumps(report, indent=2) + '\n')
    assert result.returncode == 0, name
    return result.stdout
expected = run('vm', [str(SDK / 'bin/dart'), str(SOURCE)])
run('official-compile', [str(SDK / 'bin/dart'), 'compile', 'exe', str(SOURCE), '-o', str(OUT / 'official')])
assert run('official', [str(OUT / 'official')]) == expected
for profile in ['default', 'scalar-off']:
    folder = OUT / profile
    folder.mkdir(exist_ok=True)
    e = dict(env)
    if profile == 'scalar-off':
        e['DARTFORGE_NO_SCALAR_COALESCE'] = '1'
    run(profile + '-compile', [str(CLI), 'compile-native', str(SOURCE), '--sdk', str(SDK / 'lib'),
                              '--optimize', '--memory=tracing', '--exceptions=check', '--roots=shadow',
                              '--stack-trace=none', '-o', str(folder / 'program')], e)
    actual = run(profile, [str(folder / 'program')], e)
    report['runs'][profile]['exact'] = actual == expected
    for stress in [False, True]:
        runtime = dict(e)
        if stress:
            runtime['DARTFORGE_GC_STRESS'] = '1'
        name = profile + ('-stress' if stress else '-repeat')
        actual = run(name, [str(folder / 'program')], runtime)
        report['runs'][name]['exact'] = actual == expected
    # Preserve mismatches instead of treating a baseline divergence as a pass.
    (ARCHIVE / 'validation.json').write_text(json.dumps(report, indent=2) + '\n')
report['inputs_stable'] = report['source_sha256'] == sha(SOURCE) and report['compiler_sha256'] == sha(CLI)
report['all_native_exact'] = all(v.get('exact', True) for v in report['runs'].values())
(ARCHIVE / 'validation.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({'all_native_exact': report['all_native_exact'], 'inputs_stable': report['inputs_stable']}))

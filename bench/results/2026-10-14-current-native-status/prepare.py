"""Prepare hash-bound current native/JIT artifacts without measuring performance."""
from pathlib import Path
import hashlib
import json
import os
import subprocess

root = Path.cwd()
out = root / 'bench/results/2026-10-14-current-native-status'
work = root / 'target/current-native-status'
compiler = root / 'target/current-toolchain-status/dartforge-native'
jit = root / 'target/current-toolchain-status/dartforge-jit'
sdk = Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk')
sources = {name: root / f'bench/comparison/{name}.dart' for name in ['hello', 'bench', 'server_utf8']}

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

assert not (out / 'prepared.json').exists(), 'Preserve earlier evidence'
env = {k: v for k, v in os.environ.items() if not k.startswith('DARTFORGE_')}
env.update(TMPDIR=str(root / 'target/tmp-p8-runtime'),
           DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang',
           DARTFORGE_ARC_PORT_PRUNING='1', DARTFORGE_FACTORY_RECEIVER_PROVENANCE='1',
           DARTFORGE_ARC_CHECK='0')
ledger = {'compiler': str(compiler), 'compiler_sha256': sha(compiler),
          'jit': str(jit), 'jit_sha256': sha(jit), 'sdk': str(sdk),
          'sdk_executable_sha256': sha(sdk / 'bin/dart'),
          'inputs': {str(p): sha(p) for p in [*sources.values(), Path(__file__)]},
          'runs': [], 'artifacts': {}, 'timing_claim': False, 'complete': False}

def save():
    (out / 'prepared.json').write_text(json.dumps(ledger, indent=2) + '\n')

def run(label, args, execution):
    p = subprocess.run([str(v) for v in args], env=execution, cwd=root,
                       capture_output=True, timeout=1800)
    (out / (label + '.stdout')).write_bytes(p.stdout)
    (out / (label + '.stderr')).write_bytes(p.stderr)
    ledger['runs'].append({'label': label, 'command': [str(v) for v in args],
                          'environment': {k: v for k, v in execution.items() if k.startswith('DARTFORGE_')},
                          'exit': p.returncode, 'stdout_sha256': hashlib.sha256(p.stdout).hexdigest(),
                          'stderr_sha256': hashlib.sha256(p.stderr).hexdigest()})
    save()
    assert p.returncode == 0, label
    print(label, 'complete', flush=True)

for mode in ['tracing', 'legacy-arc', 'prepared-arc', 'official']:
    execution = dict(env, DARTFORGE_ARC_PREPARED='1' if mode == 'prepared-arc' else '0')
    for name, source in sources.items():
        if name == 'server_utf8' and mode not in ['tracing', 'official']:
            continue
        folder = work / mode
        folder.mkdir(parents=True, exist_ok=True)
        binary = folder / name
        if mode == 'official':
            command = [sdk / 'bin/dart', 'compile', 'exe', source, '-o', binary]
        else:
            command = [compiler, 'compile-native', source, '--sdk', sdk / 'lib',
                       '--optimize', '--memory=' + ('tracing' if mode == 'tracing' else 'arc'),
                       '--exceptions=check', '--roots=shadow', '--stack-trace=none', '-o', binary]
        run(mode + '-' + name + '-compile', command, execution)
        ledger['artifacts'][mode + '/' + name] = {'path': str(binary), 'sha256': sha(binary), 'bytes': binary.stat().st_size}
        save()
ledger['inputs_stable'] = all(sha(p) == h for p, h in ledger['inputs'].items())
ledger['complete'] = True
assert ledger['inputs_stable'] and sha(compiler) == ledger['compiler_sha256'] and sha(jit) == ledger['jit_sha256']
save()

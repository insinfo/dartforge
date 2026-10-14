from pathlib import Path
import hashlib, json, os, shutil, subprocess

root = Path.cwd()
out = root / 'bench/results/2026-10-14-native-latin1-char-codes'
work = root / 'target/native-provenance/latin1-native-validation'
compiler = root / 'target/native-provenance/latin1-type-identity-compiler/dartforge'
sdk = Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib')
fixture = root / 'crates/emit_native/tests/fixtures/arc_ports/latin1_char_codes.dart'
http_source = root / 'bench/comparison/server_utf8.dart'
def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()
assert sha(compiler) == '528e4ceac3d471ee9784b2aea19e1cfe0e5530de8ed137feb2393d4657cdbffc'
inputs = {str(path): sha(path) for path in [fixture, http_source]}
ledger = {'compiler': str(compiler), 'compiler_sha256': sha(compiler), 'sdk': str(sdk), 'inputs': inputs, 'runs': [], 'complete': False, 'timing_measured': False}
shutil.copy2(compiler.parent / 'provenance.json', out / 'native-compiler-provenance.json')
env = os.environ.copy()
for key in ['DARTFORGE_MEMORY', 'DARTFORGE_GC_STRESS', 'DARTFORGE_ARC_PREPARED', 'DARTFORGE_NO_TABLE_PRUNING']:
    env.pop(key, None)
env.update(DARTFORGE_ARC_CHECK='1', DARTFORGE_EFFECTS='check', DARTFORGE_ARC_PORT_PRUNING='1', DARTFORGE_FACTORY_RECEIVER_PROVENANCE='1')
def save():
    (out / 'native-validation.json').write_text(json.dumps(ledger, indent=2) + '\n')
def run(label, command, execution):
    result = subprocess.run([str(value) for value in command], capture_output=True, cwd=root, env=execution, timeout=1800)
    (out / (label + '.stdout')).write_bytes(result.stdout)
    (out / (label + '.stderr')).write_bytes(result.stderr)
    row = {'label': label, 'command': [str(value) for value in command], 'environment': {key: value for key, value in execution.items() if key.startswith('DARTFORGE_')}, 'exit': result.returncode, 'stdout_sha256': hashlib.sha256(result.stdout).hexdigest(), 'stderr_sha256': hashlib.sha256(result.stderr).hexdigest()}
    ledger['runs'].append(row)
    save()
    return result, row
oracle, _ = run('fixture-vm', [sdk.parent / 'bin/dart', fixture], env)
assert oracle.returncode == 0 and not oracle.stderr
for mode in ['tracing', 'legacy-arc', 'prepared-arc']:
    execution = dict(env, DARTFORGE_ARC_PREPARED='1' if mode == 'prepared-arc' else '0')
    folder = work / mode
    folder.mkdir(parents=True, exist_ok=True)
    binary = folder / 'program'
    command = [compiler, 'compile-native', fixture, '--sdk', sdk, '--optimize', '--memory=' + ('tracing' if mode == 'tracing' else 'arc'), '--exceptions=check', '--roots=shadow', '--stack-trace=none', '-o', binary]
    result, row = run(mode + '-build', command, execution)
    if result.returncode:
        print(mode, 'build failed', result.returncode, flush=True)
        continue
    row.update(binary=str(binary), binary_bytes=binary.stat().st_size, binary_sha256=sha(binary))
    save()
    for stress in [False, True]:
        run_env = execution.copy()
        if stress: run_env['DARTFORGE_GC_STRESS'] = '1'
        result, row = run(mode + ('-stress' if stress else '-normal'), [binary], run_env)
        row['vm_exact'] = result.returncode == 0 and result.stdout == oracle.stdout and not result.stderr
        save()
        print(row['label'], 'exact VM', row['vm_exact'], flush=True)
# Build matching UTF-8 HTTP artifacts, without starting either server or timing.
for label, producer in [('before', root / 'target/native-provenance/list-reserve-after-compiler/dartforge'), ('after', compiler)]:
    if label == 'before': assert sha(producer) == '2919f3ffadb7e8d2b01b7766b82cd768482548edc0be8d58599bb902ac2d2c99'
    folder = work / ('http-utf8-' + label)
    folder.mkdir(parents=True, exist_ok=True)
    binary = folder / 'program'
    command = [producer, 'compile-native', http_source, '--sdk', sdk, '--optimize', '--memory=tracing', '--exceptions=check', '--roots=shadow', '--stack-trace=none', '-o', binary]
    result, row = run('http-utf8-' + label + '-build', command, dict(env, DARTFORGE_ARC_PREPARED='0'))
    row['compiler_sha256'] = sha(producer)
    if not result.returncode:
        row.update(binary=str(binary), binary_bytes=binary.stat().st_size, binary_sha256=sha(binary))
    save()
    print('http-utf8', label, 'compile exit', result.returncode, flush=True)
ledger['inputs_unchanged'] = all(sha(path) == digest for path, digest in inputs.items())
ledger['compiler_unchanged'] = sha(compiler) == ledger['compiler_sha256']
ledger['six_fixture_runs_exact_vm'] = sum(row.get('vm_exact', False) for row in ledger['runs']) == 6
ledger['http_binaries_built'] = all(row['exit'] == 0 for row in ledger['runs'] if row['label'].startswith('http-utf8-'))
ledger['complete'] = True
save()
assert ledger['inputs_unchanged'] and ledger['compiler_unchanged'] and ledger['six_fixture_runs_exact_vm'] and ledger['http_binaries_built']

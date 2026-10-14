import hashlib, json, os, shutil, subprocess
from pathlib import Path
root = Path.cwd()
out = root / 'bench/results/2026-10-14-arc-pending-phi'
out.mkdir(parents=True, exist_ok=True)
binaries = root / 'target/native-provenance/arc-pending-phi-binaries'
compiler = root / 'target/native-provenance/arc-pending-phi-compiler/dartforge'
sdk = Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib')
fixture = root / 'crates/emit_native/tests/fixtures/arc_ports/pending_field_phi.dart'
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
ledger = {'complete': False, 'scope': 'Exact ordinary Dart output across tracing, legacy ARC and prepared ARC; no timing or broad ARC readiness claim.', 'compiler_sha256': sha(compiler), 'fixture_sha256': sha(fixture), 'runs': [], 'timing_measured': False}
def save():
    (out / 'validation.json').write_text(json.dumps(ledger, indent=2) + '\n')
def run(label, command, env, destination=out):
    destination.mkdir(parents=True, exist_ok=True)
    result = subprocess.run([str(c) for c in command], cwd=root, env=env, capture_output=True, timeout=1200)
    (destination / (label + '.stdout')).write_bytes(result.stdout)
    (destination / (label + '.stderr')).write_bytes(result.stderr)
    row = {'label': label, 'command': [str(c) for c in command], 'environment': {k: v for k, v in env.items() if k.startswith('DARTFORGE_')}, 'exit_code': result.returncode, 'stdout_sha256': hashlib.sha256(result.stdout).hexdigest(), 'stderr_sha256': hashlib.sha256(result.stderr).hexdigest()}
    ledger['runs'].append(row)
    save()
    return result, row
base = os.environ.copy()
for key in ['DARTFORGE_MEMORY', 'DARTFORGE_GC_STRESS', 'DARTFORGE_ARC_PREPARED', 'DARTFORGE_NO_TABLE_PRUNING']:
    base.pop(key, None)
base.update({'DARTFORGE_ARC_CHECK': '1', 'DARTFORGE_EFFECTS': 'check', 'DARTFORGE_ARC_PORT_PRUNING': '1', 'DARTFORGE_FACTORY_RECEIVER_PROVENANCE': '1'})
oracle, row = run('vm', [sdk.parent / 'bin/dart', fixture], base)
assert oracle.returncode == 0 and not oracle.stderr
for mode in ['tracing', 'arc', 'prepared']:
    env = base.copy()
    if mode == 'prepared': env['DARTFORGE_ARC_PREPARED'] = '1'
    folder = binaries / mode
    folder.mkdir(parents=True, exist_ok=True)
    binary = folder / 'program'
    command = [compiler, 'compile-native', fixture, '--sdk', sdk, '--optimize', '--memory=' + ('tracing' if mode == 'tracing' else 'arc'), '--exceptions=check', '--roots=shadow', '--stack-trace=none', '-o', binary]
    result, row = run(mode + '-build', command, env)
    if result.returncode != 0:
        print(mode + ': build failed', flush=True)
        continue
    row.update({'binary_bytes': binary.stat().st_size, 'binary_sha256': sha(binary), 'binary_path': str(binary)})
    save()
    for stress in [False, True]:
        execution = env.copy()
        if stress: execution['DARTFORGE_GC_STRESS'] = '1'
        result, row = run(mode + ('-stress' if stress else '-normal'), [binary], execution)
        row['vm_exact'] = result.returncode == 0 and result.stdout == oracle.stdout and not result.stderr
        save()
        print(row['label'] + ': VM exact=' + str(row['vm_exact']), flush=True)
# Reference earlier failures without duplicating their large input inventories.
previous = root / 'bench/results/2026-10-14-arc-port-file-explorer'
retry = out / 'file-explorer'
retry.mkdir(exist_ok=True)
ledger['previous_file_explorer_evidence'] = ['bench/results/2026-10-14-arc-port-file-explorer/native-validation.json', 'bench/results/2026-10-14-arc-subtype-guard/file-explorer/prepared-build.stderr']
inputs = json.loads((previous / 'input-snapshot.json').read_text())
current = {path: {'sha256': sha(path), 'bytes': Path(path).stat().st_size} for path in inputs}
ledger['input_snapshot_reference'] = 'bench/results/2026-10-14-arc-port-file-explorer/input-snapshot.json'
ledger['input_snapshot_sha256'] = sha(previous / 'input-snapshot.json')
entry = Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui/examples/file_explorer/bin/file_explorer.dart')
env = base.copy()
env['DARTFORGE_ARC_PREPARED'] = '1'
folder = binaries / 'file-explorer-prepared'
folder.mkdir(exist_ok=True)
binary = folder / 'program'
command = [compiler, 'compile-native', entry, '--sdk', sdk, '--optimize', '--memory=arc', '--exceptions=check', '--roots=shadow', '--stack-trace=none', '-o', binary]
print('FileExplorer prepared ARC retry: compiling', flush=True)
result, row = run('prepared-build', command, env, retry)
row.update({'gui_execution_validated': False, 'inputs_unchanged': all(sha(path) == value['sha256'] for path, value in current.items())})
if result.returncode == 0:
    row.update({'binary_bytes': binary.stat().st_size, 'binary_sha256': sha(binary), 'binary_path': str(binary)})
print('FileExplorer prepared retry exit=' + str(result.returncode), flush=True)
ledger['compiler_unchanged'] = sha(compiler) == ledger['compiler_sha256']
ledger['fixture_unchanged'] = sha(fixture) == ledger['fixture_sha256']
ledger['complete'] = True
ledger['fixture_vm_exact_all_modes'] = len([r for r in ledger['runs'] if r.get('vm_exact')]) == 6
save()
assert ledger['compiler_unchanged'] and ledger['fixture_unchanged'] and row['inputs_unchanged']

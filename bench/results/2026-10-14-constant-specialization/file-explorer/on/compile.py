"""Compile the ordinary File Explorer with the frozen constant-specialization compiler on the SSD."""
from pathlib import Path
import hashlib, json, os, subprocess

root = Path.cwd()
archive = Path(__file__).resolve().parent
work = root / 'target/constant-specialization-file-explorer-on'
work.mkdir(parents=True, exist_ok=True)
sha = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
compiler = root / 'target/constant-specialization-producer-v3/dartforge'
assert sha(compiler) == 'c363ac60f6dd39338eb5dbd84dffb863767c1c8a2a2ae4c725c996b2847c4a79'
historical = json.loads((root / 'target/selector-getter-production/file-explorer/inputs.json').read_text())
before = {path: sha(path) for path in historical}
changes = [path for path, digest in historical.items() if before[path] != digest]
assert not changes, changes
environment = {key: value for key, value in os.environ.items() if not key.startswith('DARTFORGE_')}
environment.update(TMPDIR=str(work),
    DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang',
    DARTFORGE_CONSTANT_SPECIALIZATION='1', DARTFORGE_KEEP_IR='1', DARTFORGE_LINK_MAP='1')
binary = work / 'program'
assert not binary.exists(), 'Refusing to overwrite an existing result'
command = [str(compiler), 'compile-native',
    '/Volumes/ssd_dock/MyProjectsDart/dart_ui/examples/file_explorer/bin/file_explorer.dart',
    '--sdk', '/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib',
    '--packages', '/Volumes/ssd_dock/MyProjectsDart/dart_ui/examples/file_explorer/.dart_tool/package_config.json',
    '--optimize', '--memory=tracing', '--exceptions=check', '--roots=shadow', '--stack-trace=none', '-o', str(binary)]
report = {'command': command, 'producer_sha256': sha(compiler),
    'input_count': len(before), 'historical_input_changes': changes,
    'input_scope': 'Same previously recorded 11652 SDK/package/application inputs; no new dependency census claim.'}
(archive / 'validation.json').write_text(json.dumps(report, indent=2) + '\n')
with (archive / 'compile.stdout').open('wb') as stdout, (archive / 'compile.stderr').open('wb') as stderr:
    result = subprocess.run(command, env=environment, stdout=stdout, stderr=stderr, timeout=1800)
report.update(exit_code=result.returncode,
    inputs_unchanged=all(sha(path) == digest for path, digest in before.items()))
if result.returncode == 0:
    report.update(binary_bytes=binary.stat().st_size, binary_sha256=sha(binary))
report['gui_validated'] = False
(archive / 'validation.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report), flush=True)
assert result.returncode == 0 and report['inputs_unchanged']

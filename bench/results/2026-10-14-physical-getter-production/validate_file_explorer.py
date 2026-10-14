"""Compile File Explorer with one immutable producer; GUI execution belongs to the root agent."""
from pathlib import Path
import argparse
import gzip
import hashlib
import json
import os
import re
import subprocess
from urllib.parse import urljoin, urlparse, unquote

parser = argparse.ArgumentParser()
parser.add_argument('--compiler', required=True, type=Path)
parser.add_argument('--compiler-sha256', required=True)
args = parser.parse_args()
root = Path.cwd()
compiler = args.compiler.resolve()
app = Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui')
entry = app / 'examples/file_explorer/bin/file_explorer.dart'
sdk = Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib')
out = root / 'bench/results/2026-10-14-physical-getter-production/file-explorer'
work = root / 'target/native-provenance/physical-getter-file-explorer'
out.mkdir(parents=True, exist_ok=True)
work.mkdir(parents=True, exist_ok=True)


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


assert sha(compiler) == args.compiler_sha256
paths = [path for path in app.rglob('*.dart') if not any(part in ['.git', 'build', '.dart_tool'] for part in path.parts)]
paths += list(app.rglob('pubspec.yaml')) + list(app.rglob('package_config.json'))
# Snapshot the source libraries named by the actual application's package map,
# including hosted dependencies outside the dart_ui repository.
config = next((parent / '.dart_tool/package_config.json' for parent in entry.parents
               if (parent / '.dart_tool/package_config.json').exists()), None)
if config is not None:
    for package in json.loads(config.read_text()).get('packages', []):
        uri = urljoin(config.as_uri(), package['rootUri'])
        uri = urljoin(uri.rstrip('/') + '/', package.get('packageUri', 'lib/'))
        parsed = urlparse(uri)
        if parsed.scheme != 'file':
            raise ValueError('Unsupported package URI in input snapshot: ' + uri)
        library = Path(unquote(parsed.path))
        if library.exists():
            paths.extend(library.rglob('*.dart'))
inputs = {str(path.resolve()): sha(path) for path in paths}
ledger = {'compiler': str(compiler), 'compiler_sha256': sha(compiler), 'entry': str(entry),
          'sdk': str(sdk), 'source_inputs': inputs, 'runs': [], 'complete': False,
          'scope': 'Compilation and IR evidence only. No GUI execution, memory/performance timing or size improvement claim. ARC is excluded from getter devirtualization.'}
env = {key: value for key, value in os.environ.items() if not key.startswith('DARTFORGE_')}
env.update(DARTFORGE_KEEP_IR='1', DARTFORGE_ARC_PREPARED='0', DARTFORGE_ARC_CHECK='1',
           DARTFORGE_EFFECTS='check', DARTFORGE_ARC_PORT_PRUNING='1', DARTFORGE_FACTORY_RECEIVER_PROVENANCE='1',
           DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang')


def save():
    (out / 'validation.json').write_text(json.dumps(ledger, indent=2) + '\n')


for disabled in [False, True]:
    mode = 'off' if disabled else 'on'
    folder = work / mode
    folder.mkdir(exist_ok=True)
    binary = folder / 'program'
    execution = dict(env, DARTFORGE_NO_PHYSICAL_GETTER_DEVIRTUALIZATION='1' if disabled else '0')
    command = list(map(str, [compiler, 'compile-native', entry, '--sdk', sdk, '--optimize',
                            '--memory=tracing', '--exceptions=check', '--roots=shadow', '--stack-trace=none', '-o', binary]))
    print(mode + ' compile starting', flush=True)
    result = subprocess.run(command, cwd=entry.parent.parent, env=execution, capture_output=True, timeout=1800)
    (out / (mode + '-build.stdout')).write_bytes(result.stdout)
    (out / (mode + '-build.stderr')).write_bytes(result.stderr)
    row = {'mode': mode, 'command': command, 'cwd': str(entry.parent.parent), 'exit': result.returncode,
           'environment': {k: v for k, v in execution.items() if k.startswith('DARTFORGE_') or k in ['TMPDIR', 'PUB_CACHE']},
           'stdout_sha256': hashlib.sha256(result.stdout).hexdigest(), 'stderr_sha256': hashlib.sha256(result.stderr).hexdigest()}
    if result.returncode == 0:
        row.update(binary=str(binary), binary_sha256=sha(binary), binary_bytes=binary.stat().st_size)
        ir_path = folder / '.df_tmp/program.ll'
        if ir_path.exists():
            ir = ir_path.read_text()
            with gzip.open(out / (mode + '.ll.gz'), 'wb') as archive:
                archive.write(ir.encode())
            row.update(ir_sha256=sha(ir_path), selector_lookup_calls=len(re.findall(r'call ptr @df\.selector(?:_d)?\(', ir)))
    ledger['runs'].append(row)
    save()
    print(mode + ' compile terminal ' + str(result.returncode), flush=True)
ledger['inputs_unchanged'] = all(sha(path) == digest for path, digest in inputs.items())
ledger['compiler_unchanged'] = sha(compiler) == args.compiler_sha256
ledger['both_compiles_passed'] = all(row['exit'] == 0 for row in ledger['runs'])
ledger['gui_validation_pending'] = ledger['both_compiles_passed']
ledger['complete'] = True
save()
assert ledger['inputs_unchanged'] and ledger['compiler_unchanged'] and ledger['both_compiles_passed']

"""Prepare a pristine example copy and safe folders without touching original sources."""
from pathlib import Path
from urllib.parse import urljoin, urlparse, unquote
from datetime import datetime, timezone
import argparse, hashlib, json, shutil

ROOT = Path(__file__).resolve().parents[3]
ARCHIVE = Path(__file__).resolve().parent
ORIGINAL = Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui/examples/file_explorer')
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--producer', type=Path, required=True)
parser.add_argument('--producer-sha256', required=True)
options = parser.parse_args()
producer = options.producer.resolve()
assert sha(producer) == options.producer_sha256
run_id = datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')
work = ROOT / 'target/jit-file-explorer' / run_id
copy = work / 'example'
evidence = ARCHIVE / 'runs' / run_id
work.mkdir(parents=True, exist_ok=False)
evidence.mkdir(parents=True, exist_ok=False)
shutil.copytree(ORIGINAL, copy, ignore=shutil.ignore_patterns('.dart_tool', 'build', '.git'))
config_path = ORIGINAL / '.dart_tool/package_config.json'
configuration = json.loads(config_path.read_text())
original_inputs = {str(path): sha(path) for path in ORIGINAL.rglob('*')
                   if path.is_file() and path.suffix in ['.dart', '.yaml'] and '.dart_tool' not in path.parts and 'build' not in path.parts}
for package in configuration['packages']:
    root = Path(unquote(urlparse(urljoin(config_path.resolve().as_uri(), package['rootUri'])).path)).resolve()
    if package['name'] == 'file_explorer':
        root = copy
    package['rootUri'] = root.as_uri() + '/'
(copy / '.dart_tool').mkdir()
(copy / '.dart_tool/package_config.json').write_text(json.dumps(configuration, indent=2) + '\n')
fixture = work / 'safe-folder'
(fixture / 'child-folder').mkdir(parents=True)
(fixture / 'root-file.txt').write_text('isolated root fixture\n')
(fixture / 'child-folder/child-file.txt').write_text('isolated child fixture\n')
(work / 'tmp').mkdir()
entry = copy / 'bin/file_explorer.dart'
body = copy / 'lib/src/ui/folder_view.dart'
needle = "DataGridColumn(title: model.strings.name, id: 'name', width: 320)"
assert body.read_text().count(needle) == 1
record = {'status': 'prepared', 'producer': str(producer), 'producer_sha256': sha(producer),
          'work': str(work), 'example_copy': str(copy), 'entry': str(entry), 'entry_sha256': sha(entry),
          'original_entry_sha256': sha(ORIGINAL / 'bin/file_explorer.dart'),
          'original_inputs': original_inputs, 'original_package_config_sha256': sha(config_path),
          'copied_package_config_sha256': sha(copy / '.dart_tool/package_config.json'),
          'safe_folder': str(fixture), 'body_edit_file': str(body), 'body_original_sha256': sha(body),
          'body_edit': {'old': needle, 'new': "DataGridColumn(title: 'Name [JIT reload]', id: 'name', width: 320)"},
          'command': [str(producer), 'reload', str(entry), '--sdk', '/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib',
                      '--packages', str(copy / '.dart_tool/package_config.json'), '--interval', '150', '--once',
                      '--', str(fixture), '--lang=en'],
          'scope': 'Actual pristine File Explorer entry, ordinary package config override, safe fixture only; copied body mutation after opening is recorded separately. No main adapter.'}
assert record['entry_sha256'] == record['original_entry_sha256']
(evidence / 'launch.json').write_text(json.dumps(record, indent=2) + '\n')
(work / 'launch.json').write_text(json.dumps(record, indent=2) + '\n')
print(evidence / 'launch.json')

"""Prepare identical fresh-object List kernels; does not run throughput timing."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def run(command, env, folder, label):
    start = time.perf_counter()
    result = subprocess.run(command, env=env, capture_output=True, text=True, timeout=300)
    (folder / (label + '.stdout')).write_text(result.stdout)
    (folder / (label + '.stderr')).write_text(result.stderr)
    return {'command': [str(s) for s in command], 'exit_code': result.returncode,
            'wall_seconds': time.perf_counter() - start}


parser = argparse.ArgumentParser()
parser.add_argument('--compiler', type=Path, required=True, help='Frozen dartforge-jsprod binary')
parser.add_argument('--sdk', type=Path, default=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk'))
parser.add_argument('--node', type=Path, default=Path('/Volumes/ssd_dock/tools/rea/node-v24.21.0-darwin-arm64/bin/node'))
parser.add_argument('--out', type=Path, required=True, help='Unique SSD output directory')
args = parser.parse_args()
root = Path(__file__).resolve().parents[3]
source = root / 'bench/performance/fresh_list_append.dart'
args.out.mkdir(parents=True, exist_ok=False)
args.compiler = args.compiler.resolve()
args.out = args.out.resolve()
env = os.environ.copy()
env['TMPDIR'] = str(args.out)
env['DARTFORGE_SDK_LIB'] = str(args.sdk / 'lib')
env.pop('DARTFORGE_TRACE_FRESH_LIST_APPEND', None)
report = {'compiler': str(args.compiler), 'compiler_sha256': sha(args.compiler),
          'source': str(source), 'source_sha256': sha(source),
          'node': str(args.node), 'node_sha256': sha(args.node),
          'node_version': subprocess.check_output([args.node, '--version'], text=True).strip(),
          'dart': str(args.sdk / 'bin/dart'), 'dart_sha256': sha(args.sdk / 'bin/dart'),
          'profiles': {}, 'expected_checksum': 4813794,
          'scope': 'Compile wall samples include process and SDK/cache work; not a quiet compile-time benchmark. Runtime measurement requires separate isolated process execution.'}
for profile in ['forge_off', 'forge_on', 'O2', 'O4']:
    folder = args.out / profile
    folder.mkdir()
    artifact = folder / 'program.js'
    case_env = env.copy()
    if profile.startswith('forge_'):
        case_env['DARTFORGE_JSPROD_FRESH_LIST_APPEND'] = '1' if profile == 'forge_on' else '0'
        command = [args.compiler, source, '--own-sdk', '--sdk', args.sdk / 'lib', '-o', artifact]
        raw_command = command + ['--no-minify']
        raw_command[raw_command.index(artifact)] = folder / 'raw.js'
        raw = run(raw_command, case_env, folder, 'compile-raw')
        if raw['exit_code']:
            report['profiles'][profile] = {'raw': raw}
            break
        raw_text = (folder / 'raw.js').read_text()
        start = raw_text.index('function createRows(')
        body = raw_text[start:raw_text.index('\n};', start)]
        (folder / 'builder.js').write_text(body + '\n')
        raw['builder_guarded_append_sites'] = body.count('.append(')
        assert raw['builder_guarded_append_sites'] == (1 if profile == 'forge_on' else 0)
    else:
        command = [args.sdk / 'bin/dart', 'compile', 'js', '-' + profile, source, '-o', artifact]
        raw = None
    compiled = run(command, case_env, folder, 'compile')
    item = {'compile': compiled, 'raw': raw}
    if compiled['exit_code'] == 0:
        data = artifact.read_bytes()
        item.update(artifact=str(artifact), sha256=sha(artifact), bytes=len(data),
                    gzip9_bytes=len(gzip.compress(data, compresslevel=9, mtime=0)))
    report['profiles'][profile] = item
report['inputs_stable'] = sha(source) == report['source_sha256'] and sha(args.compiler) == report['compiler_sha256']
(args.out / 'prepare.json').write_text(json.dumps(report, indent=2) + '\n')
assert len(report['profiles']) == 4 and all(p['compile']['exit_code'] == 0 for p in report['profiles'].values())
assert report['inputs_stable']

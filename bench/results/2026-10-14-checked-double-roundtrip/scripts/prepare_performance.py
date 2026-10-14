"""Prepare paired format throughput artifacts; no timing is performed here."""
import argparse
import hashlib
import json
import os
import subprocess
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('--producer', required=True, type=Path)
parser.add_argument('--producer-sha256', required=True)
parser.add_argument('--work-dir', required=True, type=Path)
parser.add_argument('--output-dir', required=True, type=Path)
args = parser.parse_args()
root = Path.cwd()
work = args.work_dir.resolve()
out = args.output_dir.resolve()
work.mkdir(parents=True, exist_ok=False)
out.mkdir(parents=True, exist_ok=False)
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
previous_path = root / 'bench/results/2026-10-14-enum-pattern-identity/performance-preparation/prepared.json'
previous = json.loads(previous_path.read_text())
for path, digest in previous['inputs_before'].items():
    assert sha(path) == digest, path
producer = args.producer.resolve()
assert sha(producer) == args.producer_sha256
source = root / 'target/enum-pattern-identity/format-performance/probe.dart'
package = Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui/examples/file_explorer/.dart_tool/package_config.json')
sdk = Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib')
inputs = {str(path): sha(path) for path in [producer, source, package,
          Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui/lib/src/graphics/image/codecs/color/format.dart')]}
ledger = {'inputs_before': inputs, 'commands': [], 'artifacts': {},
          'official_reuse_ledger': str(previous_path), 'official_reuse_ledger_sha256': sha(previous_path),
          'scope': 'Compile-only preparation. Same source, packages, SDK and producer; only checked-double optout differs.'}
env = {key: value for key, value in os.environ.items() if not key.startswith('DARTFORGE_')}
env.update(TMPDIR=str(work), DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang')
def save():
    (out / 'prepared.json').write_text(json.dumps(ledger, indent=2) + '\n')
try:
    for label in ['off', 'on']:
        profile = env.copy()
        if label == 'off':
            profile['DARTFORGE_NO_CHECKED_DOUBLE_ROUNDTRIP'] = '1'
        artifact = work / label
        command = [str(producer), 'compile-native', str(source), '--sdk', str(sdk),
                   '--packages', str(package), '--optimize', '--memory=tracing',
                   '--exceptions=check', '--roots=shadow', '--stack-trace=none', '-o', str(artifact)]
        with (out / f'{label}-compile.stdout').open('wb') as stdout, (out / f'{label}-compile.stderr').open('wb') as stderr:
            run = subprocess.run(command, env=profile, stdout=stdout, stderr=stderr, timeout=1200)
        ledger['commands'].append({'command': command, 'exit_code': run.returncode,
                                  'checked_double_optout': profile.get('DARTFORGE_NO_CHECKED_DOUBLE_ROUNDTRIP')})
        save()
        assert run.returncode == 0, label
        ledger['artifacts'][label] = {'path': str(artifact), 'sha256': sha(artifact), 'bytes': artifact.stat().st_size}
    official = previous['artifacts']['official']
    assert sha(official['path']) == official['sha256']
    ledger['artifacts']['official'] = official
    for label, artifact in ledger['artifacts'].items():
        run = subprocess.run([artifact['path'], '1'], env=env, capture_output=True, text=True, timeout=60)
        (out / f'{label}-check.stdout').write_text(run.stdout)
        (out / f'{label}-check.stderr').write_text(run.stderr)
        assert run.returncode == 0 and not run.stderr
        assert run.stdout == previous['expected_one_round_stdout'], (label, run.stdout)
    ledger['inputs_unchanged'] = all(sha(path) == digest for path, digest in inputs.items())
    assert ledger['inputs_unchanged']
    ledger['result'] = 'READY_NOT_TIMED'
except BaseException as error:
    ledger['result'] = 'FAIL'
    ledger['failure'] = repr(error)
    raise
finally:
    save()

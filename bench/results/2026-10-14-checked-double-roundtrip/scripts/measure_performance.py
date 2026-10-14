"""Measure validated frozen artifacts with alternating fresh-process orders."""
import argparse
import hashlib
import json
import os
import re
import statistics
import subprocess
import time
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('--prepared', required=True, type=Path)
parser.add_argument('--output-dir', required=True, type=Path)
parser.add_argument('--rounds', type=int, default=100000)
args = parser.parse_args()
assert args.rounds > 0
manifest = json.loads(args.prepared.read_text())
assert manifest['result'] == 'READY_NOT_TIMED'
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
for path, digest in manifest['inputs_before'].items():
    assert sha(path) == digest, path
for artifact in manifest['artifacts'].values():
    assert sha(artifact['path']) == artifact['sha256']
out = args.output_dir.resolve()
out.mkdir(parents=True, exist_ok=False)
env = {key: value for key, value in os.environ.items() if not key.startswith('DARTFORGE_')}
env['TMPDIR'] = str(Path.cwd() / 'target/tmp-rea-build')
orders = [['off', 'on', 'official'], ['official', 'on', 'off'], ['on', 'off', 'official']]
rows = []
expected = None
for index, order in enumerate([orders[0], *orders]):
    for label in order:
        command = ['/usr/bin/time', '-l', manifest['artifacts'][label]['path'], str(args.rounds)]
        start = time.perf_counter_ns()
        run = subprocess.run(command, env=env, capture_output=True, text=True, timeout=60)
        wall = (time.perf_counter_ns() - start) / 1e6
        (out / f'{index}-{label}.stdout').write_text(run.stdout)
        (out / f'{index}-{label}.stderr').write_text(run.stderr)
        assert run.returncode == 0, (label, run.stderr)
        if expected is None:
            expected = run.stdout
        assert run.stdout == expected
        rss = re.search(r'(\d+)\s+maximum resident set size', run.stderr)
        assert rss
        rows.append({'order': index, 'warmup': index == 0, 'profile': label,
                     'command': command, 'exit_code': run.returncode, 'wall_ms': wall,
                     'max_rss_bytes': int(rss.group(1)), 'stdout': run.stdout})
        print(index, label, round(wall, 3), flush=True)
medians = {label: statistics.median(r['wall_ms'] for r in rows if not r['warmup'] and r['profile'] == label)
           for label in manifest['artifacts']}
result = {'rows': rows, 'rounds': args.rounds, 'calls_per_run': args.rounds * 576,
          'prepared_sha256': sha(args.prepared), 'prepared': str(args.prepared.resolve()),
          'artifacts': manifest['artifacts'], 'median_wall_ms': medians,
          'on_vs_off_speedup': medians['off'] / medians['on'],
          'on_vs_official_wall_ratio': medians['on'] / medians['official'],
          'inputs_unchanged': all(sha(path) == digest for path, digest in manifest['inputs_before'].items()),
          'scope': 'Wall time includes process startup. One warmup per profile excluded; three alternating measured runs. Identical consumed checksums.'}
assert result['inputs_unchanged']
(out / 'measurement.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(medians), flush=True)

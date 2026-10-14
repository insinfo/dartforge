"""Measure five consumed kernels in alternating fresh processes on frozen artifacts."""
import hashlib
import json
import os
import re
import statistics
import subprocess
import time
from pathlib import Path

base = Path(__file__).resolve().parent
manifest_path = base / 'bench-preparation/prepared.json'
manifest = json.loads(manifest_path.read_text())
assert manifest['complete'] and manifest['input_hashes_unchanged']
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
for path, digest in manifest['inputs'].items():
    assert sha(path) == digest
for artifact in manifest['artifacts'].values():
    assert sha(artifact['path']) == artifact['sha256']
out = base / 'bench-performance'
out.mkdir(exist_ok=False)
env = {key: value for key, value in os.environ.items() if not key.startswith('DARTFORGE_')}
env['TMPDIR'] = str(Path.cwd() / 'target/tmp-rea-build')
orders = [['off', 'on', 'official'], ['official', 'on', 'off'], ['on', 'off', 'official']]
rows = []
expected = None
for index, order in enumerate(orders):
    for label in order:
        command = ['/usr/bin/time', '-l', manifest['artifacts'][label]['path']]
        start = time.perf_counter_ns()
        run = subprocess.run(command, env=env, capture_output=True, text=True, timeout=60)
        wall = (time.perf_counter_ns() - start) / 1e6
        (out / f'{index}-{label}.stdout').write_text(run.stdout)
        (out / f'{index}-{label}.stderr').write_text(run.stderr)
        assert run.returncode == 0, run.stderr
        matches = re.findall(r'^(\w+): (\d+) (\d+) (\d+) (\d+) ms \| (-?\d+)$', run.stdout, re.M)
        assert len(matches) == 5
        results = {name: int(value) for name, *_, value in matches}
        assert set(results) == {'fib', 'trees', 'numeric', 'strings', 'json'}
        if expected is None:
            expected = results
        assert results == expected, (label, results, expected)
        rss = re.search(r'(\d+)\s+maximum resident set size', run.stderr)
        assert rss
        rows.append({'order': index, 'profile': label, 'command': command, 'exit_code': run.returncode,
                     'wall_ms': wall, 'max_rss_bytes': int(rss.group(1)), 'results': results,
                     'kernel_ms': {name: list(map(int, samples)) for name, *samples, value in matches}})
        print(index, label, round(wall, 3), flush=True)
profiles = {}
for label in manifest['artifacts']:
    selected = [row for row in rows if row['profile'] == label]
    profiles[label] = {
        'wall_ms': statistics.median(row['wall_ms'] for row in selected),
        'max_rss_bytes': statistics.median(row['max_rss_bytes'] for row in selected),
        'kernel_ms': {name: statistics.median(value for row in selected for value in row['kernel_ms'][name][1:])
                      for name in expected},
    }
ledger = {'rows': rows, 'medians': profiles, 'artifacts': manifest['artifacts'],
          'prepared_sha256': sha(manifest_path), 'inputs': manifest['inputs'],
          'inputs_unchanged': all(sha(path) == digest for path, digest in manifest['inputs'].items()),
          'scope': 'Three alternating fresh processes per profile. First of four calls per kernel discarded; process wall/RSS includes all four. Millisecond kernel timers limit resolution. Exact consumed results match upstream.'}
assert ledger['inputs_unchanged']
(out / 'measurement.json').write_text(json.dumps(ledger, indent=2) + '\n')
print(json.dumps(profiles, indent=2), flush=True)

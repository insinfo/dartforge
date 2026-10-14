"""Measure frozen format conversion executables in alternating fresh processes."""
import hashlib
import json
import os
import statistics
import subprocess
import time
from pathlib import Path

root = Path.cwd()
base = root / 'bench/results/2026-10-14-enum-pattern-identity'
manifest = json.loads((base / 'performance-preparation/prepared.json').read_text())
output = base / 'performance'
output.mkdir(exist_ok=True)
env = {k: v for k, v in os.environ.items() if not k.startswith('DARTFORGE_')}
env['TMPDIR'] = str(root / 'target/enum-pattern-identity/format-performance')
for path, digest in manifest['inputs_before'].items():
    assert hashlib.sha256(Path(path).read_bytes()).hexdigest() == digest, path
for item in manifest['artifacts'].values():
    assert hashlib.sha256(Path(item['path']).read_bytes()).hexdigest() == item['sha256']
rows = []
expected = None
orders = [['off', 'on', 'official'], ['official', 'on', 'off'], ['on', 'off', 'official']]
for index, order in enumerate([orders[0], *orders]):
    for label in order:
        command = ['/usr/bin/time', '-l', manifest['artifacts'][label]['path'], '100000']
        start = time.perf_counter_ns()
        run = subprocess.run(command, env=env, capture_output=True, text=True, timeout=60)
        wall = (time.perf_counter_ns() - start) / 1e6
        assert run.returncode == 0, (label, run.stderr)
        if expected is None:
            expected = run.stdout
        assert run.stdout == expected, (label, run.stdout, expected)
        (output / f'{index}-{label}.stdout').write_text(run.stdout)
        (output / f'{index}-{label}.stderr').write_text(run.stderr)
        rows.append({'order': index, 'warmup': index == 0, 'profile': label,
                     'command': command, 'exit_code': run.returncode, 'wall_ms': wall,
                     'stdout': run.stdout})
        print(label, index, round(wall, 3), flush=True)
medians = {label: statistics.median(r['wall_ms'] for r in rows if not r['warmup'] and r['profile'] == label)
           for label in manifest['artifacts']}
result = {'rounds': 100000, 'calls_per_run': 57600000, 'rows': rows, 'median_wall_ms': medians,
          'artifacts': manifest['artifacts'], 'inputs': manifest['inputs_before'],
          'on_vs_off_speedup': medians['off'] / medians['on'],
          'on_vs_official_wall_ratio': medians['on'] / medians['official'],
          'scope': 'Fresh-process wall time including startup. One warmup per profile excluded; three alternating measured runs. All checksums identical.'}
(output / 'measurement.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(medians), flush=True)

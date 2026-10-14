"""Compare reported GC counters, separately from wall-time measurements."""
import hashlib
import json
import os
import subprocess
from pathlib import Path

base = Path.cwd() / 'bench/results/2026-10-14-numeric-scalar-production'
manifest = json.loads((base / 'performance-preparation/prepared.json').read_text())
output = base / 'allocations'
output.mkdir(exist_ok=True)
env = {k: v for k, v in os.environ.items() if not k.startswith('DARTFORGE_')}
env['TMPDIR'] = str(Path.cwd() / 'target/numeric-scalar-format-performance')
env['DARTFORGE_GC_STATS'] = '1'
rows = []
for rounds in [1000, 10000]:
    expected = None
    for label in ['off', 'on']:
        artifact = manifest['artifacts'][label]
        assert hashlib.sha256(Path(artifact['path']).read_bytes()).hexdigest() == artifact['sha256']
        command = [artifact['path'], str(rounds)]
        run = subprocess.run(command, env=env, capture_output=True, text=True, timeout=30)
        assert run.returncode == 0, run.stderr
        if expected is None:
            expected = run.stdout
        assert run.stdout == expected
        stats = json.loads(run.stderr)['dartforge_gc']
        (output / f'{rounds}-{label}.stdout').write_text(run.stdout)
        (output / f'{rounds}-{label}.stderr').write_text(run.stderr)
        rows.append({'profile': label, 'rounds': rounds, 'calls': rounds * 576,
                     'command': command, 'exit_code': run.returncode,
                     'artifact_sha256': artifact['sha256'], 'stats': stats})
result = {'rows': rows, 'environment': {'DARTFORGE_GC_STATS': '1'},
          'scope': 'Diagnostic GC-counter executions, not timings. Counts include startup, fixture allocations and outstanding unused TLAB reservations. Heap::return_tlabs refunds unused reservations before collection; shutdown stats do not flush them. These are reported allocation counters, not exact consumed-object counts. OFF/ON output equal for each workload.'}
(output / 'measurement.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps([{'profile': r['profile'], 'rounds': r['rounds'],
                   'allocations': r['stats']['allocations'], 'collections': r['stats']['collections']}
                  for r in rows], indent=2))

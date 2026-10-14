"""Measure prepared current native programs in one coordinated quiet window."""
from pathlib import Path
import datetime
import hashlib
import json
import os
import re
import statistics
import subprocess
import time

root = Path.cwd()
out = root / 'bench/results/2026-10-14-current-native-status'
a = json.loads((out / 'prepared.json').read_text())
assert a['complete'] and a['inputs_stable']
def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()
assert all(sha(p) == h for p, h in a['inputs'].items())
assert all(sha(v['path']) == v['sha256'] for v in a['artifacts'].values())
env = {k: v for k, v in os.environ.items() if not k.startswith('DARTFORGE_')}
env.update(TMPDIR=str(root / 'target/tmp-p8-runtime'),
           DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang',
           DARTFORGE_TEST_SDK_LIB=a['sdk'] + '/lib', DARTFORGE_SDK_LIB=a['sdk'] + '/lib',
           DARTFORGE_ARC_CHECK='0')
source = root / 'bench/comparison/bench.dart'
commands = {mode: [a['artifacts'][mode + '/bench']['path']]
            for mode in ['tracing', 'legacy-arc', 'prepared-arc', 'official']}
commands['jit'] = [a['jit'], 'run', '--sdk', a['sdk'] + '/lib', source]
commands['vm'] = [a['sdk'] + '/bin/dart', source]
started_utc = datetime.datetime.now(datetime.timezone.utc).isoformat()
records = []
oracle = None
for pair in range(3):
    order = list(commands)
    if pair % 2:
        order.reverse()
    for mode in order:
        start = time.perf_counter_ns()
        p = subprocess.run(['/usr/bin/time', '-l', *map(str, commands[mode])],
                           cwd=root, env=env, capture_output=True, timeout=600)
        wall = time.perf_counter_ns() - start
        label = f'pair-{pair + 1}-{mode}'
        (out / (label + '.stdout')).write_bytes(p.stdout)
        (out / (label + '.stderr')).write_bytes(p.stderr)
        assert p.returncode == 0, label
        kernels = {}
        results = {}
        for line in p.stdout.decode().splitlines():
            m = re.fullmatch(r'(\w+): ([0-9 ]+) ms \| (.+)', line)
            assert m, line
            samples = [int(v) for v in m[2].split()]
            assert len(samples) == 4
            kernels[m[1]] = {'samples_ms': samples, 'median_after_first_ms': statistics.median(samples[1:])}
            results[m[1]] = m[3]
        assert len(kernels) == 5
        if oracle is None:
            oracle = results
        assert results == oracle, label
        rss = re.search(rb'(\d+)\s+maximum resident set size', p.stderr)
        assert rss, label
        records.append({'pair': pair + 1, 'mode': mode, 'command': list(map(str, commands[mode])),
                        'exit': p.returncode, 'wall_ns': wall, 'peak_rss_bytes': int(rss[1]),
                        'kernels': kernels, 'results': results})
        (out / 'measurement.json').write_text(json.dumps({'records': records, 'complete': False}, indent=2) + '\n')
        print(label, 'exact output', flush=True)
summary = {}
for mode in commands:
    rows = [r for r in records if r['mode'] == mode]
    summary[mode] = {'kernels_ms': {k: statistics.median(r['kernels'][k]['median_after_first_ms'] for r in rows) for k in oracle},
                     'wall_ms': statistics.median(r['wall_ns'] for r in rows) / 1e6,
                     'peak_rss_bytes': statistics.median(r['peak_rss_bytes'] for r in rows)}
report = {'started_utc': started_utc, 'finished_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'jit_policy': 'Current CLI run default: optimize=false; no optimized-guest claim', 'records': records, 'summary': summary, 'complete': True, 'all_outputs_exact': True,
          'source_hashes_stable': all(sha(p) == h for p, h in a['inputs'].items()),
          'scope': 'Three alternating process orders; four calls per kernel, discard first; median remaining calls then median processes. Millisecond kernel resolution. Process wall/RSS include JIT compilation and SDK loading, distinct from warmed kernel timings. Fresh native tracing/legacy/prepared ARC versus installed SDK3.6.2 AOT/VM.'}
(out / 'measurement.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(summary, indent=2))

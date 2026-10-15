"""Run frozen fresh-list artifacts in separate interleaved Node processes."""
import argparse
import hashlib
import json
from pathlib import Path
import statistics
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('prepared', type=Path)
parser.add_argument('--samples', type=int, default=11)
args = parser.parse_args()
report = json.loads(args.prepared.read_text())
assert report['inputs_stable']
correctness = json.loads((args.prepared.parent / 'correctness.json').read_text())
assert correctness['all_exact'], 'Run check_benchmark.py before throughput measurement'
node = Path(report['node'])
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert sha(node) == report['node_sha256']
out = args.prepared.parent
samples = {key: [] for key in report['profiles']}
stdout_hashes = {key: [] for key in report['profiles']}
for repetition in range(args.samples):
    profiles = list(report['profiles'])
    rotation = repetition % len(profiles)
    profiles = profiles[rotation:] + profiles[:rotation]
    for profile in profiles:
        artifact = Path(report['profiles'][profile]['artifact'])
        assert sha(artifact) == report['profiles'][profile]['sha256']
        started = time.perf_counter()
        result = subprocess.run([node, '-e', 'globalThis.self=globalThis;globalThis.window=globalThis;require(process.argv[1]);', artifact], capture_output=True, text=True, timeout=120)
        wall = time.perf_counter() - started
        label = f'{profile}-sample-{repetition}'
        (out / (label + '.stdout')).write_text(result.stdout)
        (out / (label + '.stderr')).write_text(result.stderr)
        assert result.returncode == 0, result.stderr
        values = dict(line.split('=', 1) for line in result.stdout.splitlines())
        assert values.keys() == {'checksum', 'elapsed_us'}, result.stdout
        assert int(values['checksum']) == report['expected_checksum'], result.stdout
        micros = int(values['elapsed_us'])
        assert micros > 0
        samples[profile].append({'kernel_microseconds': micros, 'process_wall_seconds': wall})
        stdout_hashes[profile].append(hashlib.sha256(result.stdout.encode()).hexdigest())
medians = {p: statistics.median(s['kernel_microseconds'] for s in rows) for p, rows in samples.items()}
summary = {'samples': samples, 'medians_us': medians, 'stdout_sha256': stdout_hashes,
           'forge_on_over_off': medians['forge_on'] / medians['forge_off'],
           'forge_on_over_O2': medians['forge_on'] / medians['O2'],
           'forge_on_over_O4': medians['forge_on'] / medians['O4'],
           'expected_checksum': report['expected_checksum'],
           'timer_contract': 'Uniform self/window prelude exposes performance.now to both SDK ticker implementations; fresh Node process per profile/sample.',
           'scope': 'Identical pure input workload; O4 separately omits checks/trusts types. Kernel times exclude compiler/startup; process wall includes startup and warmup. No inference about arbitrary programs or compile-time performance.'}
(out / 'measure.json').write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps({k:v for k,v in summary.items() if k not in ['samples','stdout_sha256']}, indent=2))

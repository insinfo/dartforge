from pathlib import Path
import argparse, hashlib, json, os, re, statistics, subprocess, time

parser = argparse.ArgumentParser()
parser.add_argument('--quiet-window-authorized', action='store_true', required=True)
parser.add_argument('--mode', choices=['tracing', 'legacy-arc', 'prepared-arc'], required=True)
args = parser.parse_args()
root = Path.cwd()
evidence = root / 'bench/results/2026-10-14-native-list-reserve-json'
def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()
correctness_ledgers = {label: json.loads((evidence / ('correctness-' + label + '.json')).read_text()) for label in ['before', 'after']}
assert correctness_ledgers['before']['vm_stdout_sha256'] == correctness_ledgers['after']['vm_stdout_sha256']
ledgers = {label: json.loads((evidence / ('timing-' + label + '.json')).read_text()) for label in ['before', 'after']}
assert ledgers['before']['inputs'] == ledgers['after']['inputs']
assert ledgers['before']['harness_sha256'] == ledgers['after']['harness_sha256']
assert ledgers['before']['source_sha256'] == ledgers['after']['source_sha256']
for label in ['before', 'after']:
    correctness = json.loads((evidence / ('correctness-' + label + '.json')).read_text())
    runs = correctness['artifacts'][args.mode]['runs']
    normal = runs['-normal']
    assert normal['exit'] == 0 and normal['stdout_exact_vm'] and normal['stderr_empty']
    assert correctness['compiler_sha256'] == ledgers[label]['compiler_sha256']
    assert sha(ledgers[label]['artifacts'][args.mode]['artifact']) == ledgers[label]['artifacts'][args.mode]['artifact_sha256']
for path, digest in ledgers['before']['inputs'].items():
    assert sha(path) == digest
vm_lines = (evidence / 'vm.stdout').read_text().splitlines()
expected = dict(line.split(': ', 1) for line in vm_lines[6:])
env = dict(os.environ, DARTFORGE_ARC_CHECK='1', DARTFORGE_EFFECTS='check')
for key in ['DARTFORGE_GC_STRESS', 'DARTFORGE_GC_OFF', 'DARTFORGE_ARC_TRACE', 'DARTFORGE_MEMORY', 'DARTFORGE_ARC_PREPARED']:
    env.pop(key, None)
diagnostic_scope = 'Diagnostic-enabled before/after comparison only: ARC_CHECK=1 and EFFECTS=check. Includes ARC synchronization audits; not production throughput, not a cross-mode ARC comparison.'
records = []
for pair, order in enumerate([['before', 'after'], ['after', 'before'], ['before', 'after']], 1):
    for label in order:
        artifact = ledgers[label]['artifacts'][args.mode]['artifact']
        started = time.perf_counter_ns()
        run = subprocess.run([artifact], capture_output=True, env=env, timeout=1800)
        elapsed = time.perf_counter_ns() - started
        prefix = evidence / (args.mode + '-pair-' + str(pair) + '-' + label)
        Path(str(prefix) + '.stdout').write_bytes(run.stdout)
        Path(str(prefix) + '.stderr').write_bytes(run.stderr)
        assert run.returncode == 0 and run.stderr == b''
        lines = run.stdout.decode().splitlines()
        assert lines[:6] == vm_lines[:6]
        kernels = {}
        for line in lines[6:]:
            match = re.fullmatch(r'([^:]+): ([0-9 ]+) us \| (.+)', line)
            assert match and expected[match[1]] == match[3]
            rounds = [int(value) for value in match[2].split()]
            assert len(rounds) == (4 if match[1] in ['decode_large', 'encode_large'] else 6)
            kernels[match[1]] = {'rounds_us': rounds, 'median_after_first_us': statistics.median(rounds[1:]), 'result': match[3]}
        assert set(kernels) == set(expected)
        records.append({'pair': pair, 'label': label, 'wall_ns': elapsed, 'kernels': kernels})
        (evidence / ('measurement-' + args.mode + '.json')).write_text(json.dumps({'mode': args.mode, 'diagnostic_scope': diagnostic_scope, 'environment': {key: value for key, value in env.items() if key.startswith('DARTFORGE_')}, 'records': records, 'timing_scope': 'Fresh process wall includes deterministic setup and all kernels; kernel medians exclude first round. Three alternating pairs, ARC checks enabled, stress disabled.'}, indent=2) + '\n')

summary = {}
for name in ['process_wall'] + list(expected):
    pairs = []
    for pair in range(1, 4):
        values = {row['label']: row['wall_ns'] if name == 'process_wall' else row['kernels'][name]['median_after_first_us'] for row in records if row['pair'] == pair}
        pairs.append({'pair': pair, 'before': values['before'], 'after': values['after'], 'after_over_before': values['after'] / values['before']})
    summary[name] = {'scope': diagnostic_scope, 'pairs': pairs, 'median_after_over_before': statistics.median(row['after_over_before'] for row in pairs)}
(evidence / ('summary-' + args.mode + '.json')).write_text(json.dumps(summary, indent=2) + '\n')

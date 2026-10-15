"""Require exact normalized output for frozen artifacts before throughput runs."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('prepared', type=Path)
args = parser.parse_args()
prepared = json.loads(args.prepared.read_text())
sha = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
assert prepared['inputs_stable']
assert sha(prepared['source']) == prepared['source_sha256']
assert sha(prepared['compiler']) == prepared['compiler_sha256']
assert sha(prepared['node']) == prepared['node_sha256']
out = args.prepared.parent
cases = [('VM', [prepared['dart'], 'run', prepared['source']], None)]
for profile, metadata in prepared['profiles'].items():
    artifact = Path(metadata['artifact'])
    assert sha(artifact) == metadata['sha256']
    command = [prepared['node'], '-e', 'globalThis.self=globalThis;globalThis.window=globalThis;require(process.argv[1]);', str(artifact)]
    cases.append((profile, command, metadata['sha256']))
    raw = artifact.parent / 'raw.js'
    if raw.exists():
        command = command[:-1] + [str(raw)]
        cases.append((profile + '_raw', command, sha(raw)))
expected = f"checksum={prepared['expected_checksum']}\nelapsed_us=<measured>\n"
runs = []
for name, command, artifact_hash in cases:
    result = subprocess.run(command, env=dict(os.environ, TMPDIR=str(out)), capture_output=True, text=True, timeout=120)
    (out / (name + '-check.stdout')).write_text(result.stdout)
    (out / (name + '-check.stderr')).write_text(result.stderr)
    values = dict(line.split('=', 1) for line in result.stdout.splitlines()) if result.returncode == 0 else {}
    exact = result.returncode == 0 and not result.stderr and values.keys() == {'checksum', 'elapsed_us'} and values['checksum'] == str(prepared['expected_checksum']) and values['elapsed_us'].isdigit() and int(values['elapsed_us']) > 0
    normalized = f"checksum={values.get('checksum')}\nelapsed_us=<measured>\n" if exact else result.stdout
    exact = exact and normalized == expected
    runs.append({'profile': name, 'command': command, 'exit_code': result.returncode,
                 'artifact_sha256': artifact_hash, 'stdout_sha256': hashlib.sha256(result.stdout.encode()).hexdigest(),
                 'normalized_output': normalized, 'exact': exact})
report = {'runs': runs, 'all_exact': all(r['exact'] for r in runs), 'expected_normalized_output': expected,
          'scope': 'Elapsed fields validated but not summarized as performance. VM/raw/min/O2/O4 share identical source and checksum; only one explicit clock field is normalized. O4 retains its distinct omit-check policy.'}
(out / 'correctness.json').write_text(json.dumps(report, indent=2) + '\n')
assert report['all_exact'], report

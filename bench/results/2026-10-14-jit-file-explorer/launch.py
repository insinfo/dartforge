"""Launch the prepared real app; this process waits for its actual terminal status."""
from pathlib import Path
import argparse, hashlib, json, os, subprocess
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('launch_json', type=Path)
options = parser.parse_args()
path = options.launch_json.resolve()
record = json.loads(path.read_text())
work = Path(record['work'])
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(Path(record['producer'])) == record['producer_sha256']
assert sha(Path(record['entry'])) == record['entry_sha256']
env = {key: value for key, value in os.environ.items() if not key.startswith('DARTFORGE_')}
env.update(TMPDIR=str(work / 'tmp'),
           DARTFORGE_SDK_LIB='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib',
           DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang',
           LLVM_SYS_221_PREFIX='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64',
           SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk')
with (path.parent / 'stdout').open('wb') as stdout, (path.parent / 'stderr').open('wb') as stderr:
    child = subprocess.Popen(record['command'], cwd=record['example_copy'], env=env, stdout=stdout, stderr=stderr)
    record.update(status='running', supervisor_pid=child.pid)
    path.write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps({'pid': child.pid, 'evidence': str(path.parent)}), flush=True)
    code = child.wait()
record.update(status='terminal', exit=code,
              original_inputs_unchanged=all(sha(Path(name)) == digest for name, digest in record['original_inputs'].items()),
              producer_unchanged=sha(Path(record['producer'])) == record['producer_sha256'])
path.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps({'exit': code, 'original_inputs_unchanged': record['original_inputs_unchanged']}), flush=True)
raise SystemExit(code)

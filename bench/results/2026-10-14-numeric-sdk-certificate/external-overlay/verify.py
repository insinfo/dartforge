"""Check source-cache invalidation across external overlay edits, using one producer."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--producer', type=Path, required=True)
parser.add_argument('--producer-sha256', required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
root = Path.cwd()
work = args.output.resolve()
work.mkdir(parents=True, exist_ok=False)
producer = args.producer.resolve()
sdk = Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk')
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert sha(producer) == args.producer_sha256
overlay = work / 'overlay'
shutil.copytree(root / 'native_sdk', overlay, copy_function=shutil.copyfile)
mapping = overlay / 'libraries.json'
data = json.loads(mapping.read_text())
data['dartforge_native']['replaces']['_internal/vm/lib/object_patch.dart'] = '../external-object.dart'
mapping.write_text(json.dumps(data, indent=2) + '\n')
external = work / 'external-object.dart'
canonical = (sdk / 'lib/_internal/vm/lib/object_patch.dart').read_text()
old = 'throw new NoSuchMethodError.withInvocation(this, invocation);'
assert canonical.count(old) == 1
entry = work / 'main.dart'
entry.write_text("""@pragma('vm:never-inline')
Object roundTrip(dynamic value) {
  final double scalar = value;
  return scalar;
}
void main() {
  dynamic original = 1.25;
  dynamic returned = roundTrip(original);
  try {
    print(original.missingMethod() == returned.missingMethod());
  } catch (_) {
    print('throws');
  }
}
""")
env = {k: v for k, v in os.environ.items() if not k.startswith('DARTFORGE_')}
env.update(TMPDIR=str(work), DARTFORGE_NATIVE_SDK=str(overlay), DARTFORGE_KEEP_IR='1',
           DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang')
inputs = {str(p): sha(p) for p in [producer, mapping, entry]}
ledger = {'producer_sha256': sha(producer), 'unchanged_inputs': inputs, 'cases': [],
          'scope': 'One compiler, SDK, overlay mapping and source; only an external replacement file changes. Modified observer must preserve boxing; restored SDK must throw and may reuse the checked double box. No timing claim.'}

def save():
    (work / 'validation.json').write_text(json.dumps(ledger, indent=2) + '\n')

for phase, text, expected, boxes in [
    ('modified', canonical.replace(old, 'return _getHash(this);'), 'false\n', 1),
    ('restored', canonical, 'throws\n', 0),
]:
    external.write_text(text)
    folder = work / phase
    folder.mkdir()
    command = [str(producer), 'compile-native', str(entry), '--sdk', str(sdk / 'lib'),
               '--optimize', '--memory=tracing', '--exceptions=check', '--roots=shadow',
               '--stack-trace=none', '-o', str(folder / 'program')]
    row = {'phase': phase, 'selected_source_sha256': sha(external), 'command': command}
    ledger['cases'].append(row)
    save()
    with (folder / 'compile.stdout').open('wb') as so, (folder / 'compile.stderr').open('wb') as se:
        result = subprocess.run(command, env=env, stdout=so, stderr=se, timeout=1200)
    row['compile_exit'] = result.returncode
    save()
    assert result.returncode == 0
    result = subprocess.run([str(folder / 'program')], env=env, capture_output=True, timeout=60)
    (folder / 'run.stdout').write_bytes(result.stdout)
    (folder / 'run.stderr').write_bytes(result.stderr)
    ir = (folder / '.df_tmp/program.ll').read_text()
    bodies = re.findall(r'define[^\n]*@[^\n]*\.roundTrip\([^\n]*\).*?\n\}', ir, re.S)
    assert len(bodies) == 1
    (folder / 'roundTrip.ll').write_text(bodies[0] + '\n')
    row.update(run_exit=result.returncode, stdout=result.stdout.decode(), stderr=result.stderr.decode(),
               artifact_sha256=sha(folder / 'program'),
               double_boxes=len(re.findall(r'call[^\n]*@df.box_double\(', bodies[0])))
    save()
    print(phase, row['run_exit'], row['stdout'].strip(), 'BOX', row['double_boxes'], flush=True)
    assert result.returncode == 0 and result.stdout.decode() == expected and not result.stderr
    assert row['double_boxes'] == boxes
ledger['inputs_unchanged'] = all(sha(Path(p)) == digest for p, digest in inputs.items())
ledger['passed'] = ledger['inputs_unchanged']
save()
assert ledger['passed']
print('PASS external overlay invalidation', flush=True)

"""Build paired production tracing artifacts and inspect IR/link maps without timing."""
import gzip
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

root = Path.cwd()
out = root / 'bench/results/2026-10-14-numeric-scalar-production/bench-preparation'
work = root / 'target/numeric-scalar-bench'
work.mkdir(parents=True, exist_ok=True)
producer = root / 'target/numeric-scalar-producer/dartforge'
expected = '1d2dadbc030f9da15f4672900ad1f9aab1ea736edb0acff6be82174b8d620655'
source = root / 'bench/comparison/bench.dart'
sdk = Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk')
previous_path = root / 'bench/results/2026-10-14-current-native-status/prepared.json'
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert sha(producer) == expected
previous = json.loads(previous_path.read_text())
assert previous['inputs'][str(source)] == sha(source)
assert previous['sdk'] == str(sdk) and previous['sdk_executable_sha256'] == sha(sdk / 'bin/dart')
official = previous['artifacts']['official/bench']
assert sha(official['path']) == official['sha256']
official_command = next(r for r in previous['runs'] if r['label'] == 'official-bench-compile')
assert official_command['exit'] == 0 and official_command['command'] == [str(sdk / 'bin/dart'), 'compile', 'exe', str(source), '-o', official['path']]
assert not (out / 'prepared.json').exists(), 'Preserve existing producer evidence'
inputs = {str(source): sha(source), str(sdk / 'bin/dart'): sha(sdk / 'bin/dart'), str(producer): sha(producer)}
ledger = {'compiler': str(producer), 'compiler_sha256': expected, 'inputs': inputs,
          'official_reuse_ledger': str(previous_path), 'official_reuse_ledger_sha256': sha(previous_path),
          'official_command': official_command,
          'scope': 'Production tracing OFF/ON scalar coalesce; checked-double remains default ON in both. No executions/timers during preparation. Reused official Dart3.6.2 AOT has exact benchmark input, SDK executable and compile command hashes.',
          'runs': [], 'artifacts': {'official': official}, 'ir': {}, 'complete': False, 'timings_measured': False}
for name in ['source.json', 'provenance.json']:
    p = producer.parent / name
    shutil.copy2(p, out / ('compiler-' + name))
for name in ['runtime-archives.json']:
    shutil.copy2(out.parent / 'scalar' / name, out / name)
env = {k:v for k,v in os.environ.items() if not k.startswith('DARTFORGE_')}
env.update(TMPDIR=str(work), DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang',
           LLVM_SYS_221_PREFIX='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64',
           DARTFORGE_SDK_LIB=str(sdk / 'lib'), DARTFORGE_KEEP_IR='1', DARTFORGE_LINK_MAP='1',
           DARTFORGE_ARC_CHECK='0', DARTFORGE_ARC_PREPARED='0',
           DARTFORGE_ARC_PORT_PRUNING='1', DARTFORGE_FACTORY_RECEIVER_PROVENANCE='1')

def save():
    (out / 'prepared.json').write_text(json.dumps(ledger,indent=2) + '\n')

for mode in ['off', 'on']:
    folder = work / mode
    folder.mkdir(exist_ok=True)
    binary = folder / 'program'
    execution = env.copy()
    if mode == 'off':
        execution['DARTFORGE_NO_SCALAR_COALESCE'] = '1'
    command = list(map(str, [producer, 'compile-native', source, '--sdk', sdk / 'lib',
                    '--optimize', '--memory=tracing', '--exceptions=check', '--roots=shadow',
                    '--stack-trace=none', '-o', binary]))
    print(mode + '-compile starting', flush=True)
    result = subprocess.run(command, env=execution, cwd=root, capture_output=True, timeout=1800)
    (out / (mode + '-compile.stdout')).write_bytes(result.stdout)
    (out / (mode + '-compile.stderr')).write_bytes(result.stderr)
    ledger['runs'].append({'label': mode, 'command': command, 'exit': result.returncode,
        'environment': {k:v for k,v in execution.items() if k.startswith('DARTFORGE_') or k in ['TMPDIR', 'LLVM_SYS_221_PREFIX']},
        'stdout_sha256': hashlib.sha256(result.stdout).hexdigest(), 'stderr_sha256': hashlib.sha256(result.stderr).hexdigest()})
    save()
    print(mode + '-compile terminal ' + str(result.returncode), flush=True)
    assert result.returncode == 0
    ledger['artifacts'][mode] = {'path':str(binary), 'bytes':binary.stat().st_size, 'sha256':sha(binary)}
    path = folder / '.df_tmp/program.ll'
    ir = path.read_text()
    bodies = re.findall(r'^define [^\n]*@[^\s(]*\.Node\.count\([^\n]*\{\n.*?^\}', ir, re.M | re.S)
    assert len(bodies) == 1
    body = bodies[0]
    symbol = re.search(r'@([^\s(]+)\(',body).group(1)
    (out / (mode + '-node-count.ll')).write_text(body + '\n')
    ledger['ir'][mode] = {'path':str(path), 'sha256':sha(path), 'node_symbol':symbol,
        'integer_boxes':len(re.findall(r'call i64 @df\.box_int\(',body)),
        'integer_unboxes':len(re.findall(r'call i64 @df\.unbox_int\(',body)),
        'recursive_calls':len(re.findall(r'call i64 @[^\s(]*\.Node\.count\(',body)),
        'pending_checks':len(re.findall(r'load i8, ptr %ctx, align 8',body))}
    linkmap = binary.with_suffix('.map')
    lines = [line for line in linkmap.read_text().splitlines() if line.split() and line.split()[-1] == '_' + symbol]
    (out / (mode + '-node-count-map.txt')).write_text('\n'.join(lines) + '\n')
    assert len(lines) == 1, 'Exact typed Node.count map symbol required'
    ledger['artifacts'][mode]['node_count_text_bytes'] = int(lines[0].split()[1],16)
    ledger['artifacts'][mode]['link_map_sha256'] = sha(linkmap)
    save()
ledger['input_hashes_unchanged'] = all(sha(path) == digest for path,digest in inputs.items())
a,b = ledger['ir']['off'],ledger['ir']['on']
ledger['actual_two_pairs_removed'] = a['integer_boxes'] == a['integer_unboxes'] == 2 and b['integer_boxes'] == b['integer_unboxes'] == 0 and a['recursive_calls'] == b['recursive_calls'] == 2 and a['pending_checks'] == b['pending_checks'] == 4
ledger['complete'] = True
save()
assert ledger['input_hashes_unchanged'] and ledger['actual_two_pairs_removed']

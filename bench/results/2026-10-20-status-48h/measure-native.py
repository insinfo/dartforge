"""Compare current optimized macOS AOT outputs with Dart 3.6.2."""
import hashlib
import json
import os
from pathlib import Path
import re
import statistics
import subprocess
import time

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
WORK = ROOT / 'target/status-48h-native'
WORK.mkdir(parents=True, exist_ok=True)
SDK = Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk')
LLVM = Path('/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64')
FORGE = ROOT / 'target/test-tools/dartforge-explicit-sdk'
ENV = dict(os.environ, DARTFORGE_LLVM_DIR=str(LLVM),
           DARTFORGE_SDK_LIB=str(SDK / 'lib'),
           DARTFORGE_NATIVE_CACHE=str(ROOT / 'target/rpo-native-cache'),
           SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk',
           MACOSX_DEPLOYMENT_TARGET='15.0', PATH=str(LLVM / 'bin') + ':' + os.environ['PATH'])
for key in ['DARTFORGE_SDK_DLL', 'DARTFORGE_GC_STRESS']:
    ENV.pop(key, None)
records = []


def run(command):
    start = time.monotonic()
    process = subprocess.run([str(x) for x in command], cwd=ROOT, env=ENV,
                             capture_output=True, timeout=900)
    row = dict(command=[str(x) for x in command], exit=process.returncode,
               seconds=time.monotonic() - start,
               stdout=process.stdout.decode(errors='replace'),
               stderr=process.stderr.decode(errors='replace'), load=os.getloadavg())
    records.append(row)
    (OUT / 'native-commands.json').write_text(json.dumps(records, indent=2) + '\n')
    if process.returncode:
        raise RuntimeError(json.dumps(row))
    return row


report = dict(compiler_sha256=hashlib.sha256(FORGE.read_bytes()).hexdigest(),
              method='Optimized AOT, default exception policy. Five alternating process runs, four samples/kernel/run; all checksums must match. Compiler is a debug-built driver; compile durations are metadata, not a compiler-speed comparison.',
              programs={})
for program in ['hello', 'bench']:
    source = ROOT / f'bench/comparison/{program}.dart'
    binaries = {tool: WORK / f'{program}-{tool}' for tool in ['forge', 'dart']}
    run([FORGE, 'compile-native', source, '--optimize', '--sdk', SDK / 'lib', '-o', binaries['forge']])
    run([SDK / 'bin/dart', 'compile', 'exe', source, '-o', binaries['dart']])
    entry = dict(source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(), tools={})
    for tool, path in binaries.items():
        entry['tools'][tool] = dict(bytes=path.stat().st_size,
                                   sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                                   samples={}, checksums={})
    for rep in range(5):
        for tool in (['forge', 'dart'] if rep % 2 == 0 else ['dart', 'forge']):
            result = run([binaries[tool]])
            data = entry['tools'][tool]
            for line in result['stdout'].splitlines():
                match = re.fullmatch(r'(\w+): ([\d ]+) ms \| (.+)', line)
                if match:
                    kernel, samples, checksum = match.groups()
                    data['samples'].setdefault(kernel, []).extend(map(int, samples.split()))
                    if kernel in data['checksums']:
                        assert data['checksums'][kernel] == checksum
                    data['checksums'][kernel] = checksum
            if program == 'hello':
                assert result['stdout'] == 'hello\n'
    assert entry['tools']['forge']['checksums'] == entry['tools']['dart']['checksums']
    for data in entry['tools'].values():
        data['minimum_ms'] = {k: min(v) for k, v in data['samples'].items()}
        data['median_ms'] = {k: statistics.median(v) for k, v in data['samples'].items()}
    report['programs'][program] = entry
    (OUT / 'native-measurements.json').write_text(json.dumps(report, indent=2) + '\n')
    print(program, json.dumps(entry), flush=True)

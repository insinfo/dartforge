"""Compile current unchanged kernels; execution timing requires a separate grant."""
from pathlib import Path
import gzip, hashlib, json, os, subprocess, time
ROOT = Path.cwd()
OUT = ROOT / 'target/current-js-status'
SDK = Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk')
COMPILER = OUT / 'compiler/dartforge-jsprod'
CASES = ['json', 'typed_data', 'objects_in_collections', 'calls']
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
ledger = {'compiler': str(COMPILER), 'compiler_sha256': sha(COMPILER), 'inputs': {}, 'artifacts': {}, 'scope': 'Compile and untimed correctness snapshots only. Original common.dart retained for timing artifacts.'}
correct = OUT / 'correctness'; correct.mkdir(exist_ok=True)
for case in CASES:
    source = ROOT / f'bench/performance/{case}.dart'
    ledger['inputs'][str(source)] = sha(source)
    (correct / source.name).write_bytes(source.read_bytes())
common = ROOT / 'bench/performance/common.dart'
ledger['inputs'][str(common)] = sha(common)
(correct / 'common.dart').write_text("void measure(String name, Object? Function() kernel, {int rounds = 6}) { print('$name: ${kernel()}'); }\n")
ledger['correctness_harness_sha256'] = sha(correct / 'common.dart')
for scope in ['correctness', 'timing']:
    for case in CASES:
        source = correct / f'{case}.dart' if scope == 'correctness' else ROOT / f'bench/performance/{case}.dart'
        profiles = {}
        for profile in ['raw', 'min', 'O2']:
            folder = OUT / scope / case / profile; folder.mkdir(parents=True, exist_ok=True)
            artifact = folder / 'program.js'
            command = ([str(SDK/'bin/dart'), 'compile', 'js', '-O2', str(source), '-o', str(artifact)] if profile == 'O2' else [str(COMPILER), str(source), '--sdk', str(SDK/'lib'), '-o', str(artifact)] + (['--no-minify'] if profile == 'raw' else []))
            start = time.monotonic()
            result = subprocess.run(command, capture_output=True, text=True, timeout=240, env=dict(os.environ, TMPDIR=str(OUT)))
            elapsed = time.monotonic() - start
            (folder/'compile.stdout').write_text(result.stdout); (folder/'compile.stderr').write_text(result.stderr)
            item = {'command': command, 'exit': result.returncode, 'compile_wall_seconds': elapsed, 'compile_time_scope': 'Single CLI process wall time; separate from Rust host build, no repeated quiet-window compile-time benchmark.'}
            if result.returncode == 0:
                data = artifact.read_bytes()
                item.update(artifact=str(artifact), sha256=sha(artifact), bytes=len(data), gzip9_bytes=len(gzip.compress(data, compresslevel=9, mtime=0)))
            profiles[profile] = item
            print(scope, case, profile, result.returncode, flush=True)
        ledger['artifacts'][scope+'.'+case] = profiles
assert all(sha(p) == h for p,h in ledger['inputs'].items())
assert sha(COMPILER) == ledger['compiler_sha256']
ledger['inputs_stable'] = True
(OUT/'artifacts.json').write_text(json.dumps(ledger, indent=2)+'\n')

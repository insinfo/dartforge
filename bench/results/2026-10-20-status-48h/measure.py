"""Measure fresh-process JS compilation and verify output against Dart 3.6.2."""
import gzip
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
WORK = ROOT / 'target/status-48h-measurements'
SDK = Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk')
NODE = '/Volumes/ssd_dock/tools/rea/node-v24.21.0-darwin-arm64/bin/node'
FORGE = ROOT / 'target/status-48h-build/release/dartforge'
PROD = FORGE.with_name('dartforge-jsprod')
RUNTIME = ROOT / 'runtime/ddc/dart_sdk.js'
ENV = dict(os.environ, DARTFORGE_SDK_LIB=str(SDK / 'lib'),
           DARTFORGE_DART_SDK_JS=str(RUNTIME))
WORK.mkdir(parents=True, exist_ok=True)
rows = []
commands = []


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(command, cwd=ROOT):
    start = time.monotonic()
    result = subprocess.run([str(x) for x in command], cwd=cwd, env=ENV,
                            capture_output=True, timeout=300)
    record = dict(command=[str(x) for x in command], cwd=str(cwd),
                  seconds=time.monotonic() - start, exit=result.returncode,
                  stdout=result.stdout.decode(errors='replace'),
                  stderr=result.stderr.decode(errors='replace'),
                  load=os.getloadavg())
    commands.append(record)
    (OUT / 'commands.json').write_text(json.dumps(commands, indent=2) + '\n')
    if result.returncode:
        raise RuntimeError(json.dumps(record))
    return record


def sizes(paths):
    paths = sorted(set(paths))
    return dict(bytes=sum(p.stat().st_size for p in paths),
                gzip9_bytes=sum(len(gzip.compress(p.read_bytes(), compresslevel=9, mtime=0))
                                for p in paths),
                files=[dict(path=str(p), bytes=p.stat().st_size, sha256=digest(p))
                       for p in paths])


for source in ['bench/comparison/hello.dart', 'corpus/js/40_classes_basico.dart',
               'corpus/js/120_convert_json.dart']:
    src = ROOT / source
    name = src.stem
    oracle = run([SDK / 'bin/dart', src])['stdout']
    dirs = {tool: WORK / name / tool for tool in ['dev', 'ddc', 'prod', 'o2']}
    for directory in dirs.values():
        directory.mkdir(parents=True, exist_ok=True)
    recipes = {
        'dev': [FORGE, 'compile-js', src, '--sdk', SDK / 'lib', '-o', dirs['dev']],
        'ddc': [SDK / 'bin/dart', SDK / 'bin/snapshots/dartdevc.dart.snapshot',
                '--modules=es6', '-o', dirs['ddc'] / 'app.js', src],
        'prod': [PROD, src, '--sdk', SDK / 'lib', '-o', dirs['prod'] / 'app.js'],
        'o2': [SDK / 'bin/dart', 'compile', 'js', '-O2', '-o', dirs['o2'] / 'app.js', src],
    }
    samples = {tool: [] for tool in recipes}
    # One unmeasured warmup per tool; process startup remains in each sample.
    for command in recipes.values():
        run(command)
    for rep in range(3):
        order = ['dev', 'ddc', 'prod', 'o2']
        if rep % 2:
            order.reverse()
        for tool in order:
            samples[tool].append(run(recipes[tool])['seconds'])
    for tool, directory in dirs.items():
        files = [p for p in list(directory.rglob('*.js')) + list(directory.rglob('*.mjs'))
                 if p.name != 'dart_sdk.js']
        if tool == 'ddc':
            js = (directory / 'app.js').read_text().replace("from 'dart_sdk.js'", "from '" + RUNTIME.as_uri() + "'")
            match = re.search(r'^export \{([^}]+)\}', js, re.M)
            export = match[1].split(',')[0].strip().split(' as ')[-1]
            (directory / 'app.js').write_text(js)
            (directory / 'main.mjs').write_text(f"import {{ {export} as m }} from './app.js';\nm.main();\n")
            files.append(directory / 'main.mjs')
            entry = directory / 'main.mjs'
        elif tool == 'dev':
            entry = directory / 'main.mjs'
        else:
            entry = directory / 'app.js'
        result = run([NODE, entry])
        if result['stdout'] != oracle:
            raise RuntimeError(f'{name}/{tool}: output differs from VM')
        total_files = files + ([RUNTIME] if tool in ['dev', 'ddc'] else [])
        row = dict(program=source, source_sha256=digest(src), tool=tool,
                   seconds=samples[tool], median_seconds=statistics.median(samples[tool]),
                   application=sizes(files), deployable=sizes(total_files),
                   matches_vm=True, stdout_sha256=hashlib.sha256(oracle.encode()).hexdigest())
        rows.append(row)
        (OUT / 'measurements.json').write_text(json.dumps(rows, indent=2) + '\n')
        print(name, tool, round(row['median_seconds'], 4), row['deployable']['bytes'], flush=True)

(OUT / 'bindings.json').write_text(json.dumps(dict(
    head=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
    sdk_version=(SDK / 'version').read_text().strip(),
    node_version=run([NODE, '--version'])['stdout'].strip(),
    binaries={str(p): digest(p) for p in [FORGE, PROD, SDK / 'bin/dart',
                                        SDK / 'bin/snapshots/dartdevc.dart.snapshot', RUNTIME]},
    method='One warmup, three fresh processes/tool/input; reverse order every other round; warm filesystem; source maps excluded; gzip9 summed per file; SDK runtime included once for dev/DDC; no browser, daemon or incremental build measurement.'
), indent=2) + '\n')

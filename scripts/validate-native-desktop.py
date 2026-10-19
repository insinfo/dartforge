#!/usr/bin/env python3
"""Build/run the real dart_ui File Explorer and its native menu smoke with DartForge.

Run inside a graphical user session. The script never calls the upstream Dart
compiler, edits application sources, or sends input to another application.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', type=Path, required=True)
    parser.add_argument('--ui-root', type=Path, required=True)
    parser.add_argument('--sdk', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--export-target', type=Path)
    parser.add_argument('--backend')
    parser.add_argument('--presentation')
    parser.add_argument('--mode', choices=['all', 'aot', 'jit'], default='all')
    options = parser.parse_args()
    compiler = options.compiler.resolve()
    ui = options.ui_root.resolve()
    example = ui / 'examples/file_explorer'
    out = options.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    environment = dict(os.environ, DARTFORGE_SDK_LIB=str(options.sdk.resolve()),
                       DART_UI_FONTS=str(ui / 'assets/fonts'))
    records = []
    report = {'platform': platform.platform(),
              'compiler': str(compiler),
              'compiler_sha256': hashlib.sha256(compiler.read_bytes()).hexdigest(),
              'mode': options.mode, 'runs': records}

    def run(name, command, marker=None):
        command = [str(value) for value in command]
        start = time.monotonic()
        try:
            process = subprocess.run(command, cwd=example, env=environment,
                                     capture_output=True, timeout=1200)
            stdout, stderr, code = process.stdout, process.stderr, process.returncode
        except subprocess.TimeoutExpired as error:
            stdout, stderr, code = error.stdout or b'', error.stderr or b'', None
        (out / (name + '.stdout')).write_bytes(stdout)
        (out / (name + '.stderr')).write_bytes(stderr)
        passed = code == 0 and (marker is None or marker in stdout.decode(errors='replace'))
        record = dict(name=name, command=command, exit=code, passed=passed,
                      seconds=round(time.monotonic() - start, 3))
        records.append(record)
        (out / ('results-' + options.mode + '.json')).write_text(
            json.dumps(report, indent=2) + '\n')
        print(json.dumps(record), flush=True)
        if not passed:
            raise RuntimeError(name + ': ' + stderr.decode(errors='replace')[-3000:])

    if options.export_target:
        run('export-target', [compiler, 'export-target', options.export_target.resolve()])
    common = ['--sdk', options.sdk.resolve(), '--packages',
              example / '.dart_tool/package_config.json']
    window = ['--backend=' + options.backend] if options.backend else []
    if options.presentation:
        window.append('--presentation=' + options.presentation)
    else:
        window.append('--cpu')
    for name, entry in [('explorer', example / 'bin/file_explorer.dart'),
                        ('context-smoke', example / 'tool/context_menu_retarget_smoke.dart')]:
        suffix = '.exe' if os.name == 'nt' else ''
        executable = out / (name + suffix)
        args = window if name == 'context-smoke' else [example, '--lang=pt', '--frames=2', *window]
        marker = 'CONTEXT_RETARGET=PASS' if name == 'context-smoke' else None
        if options.mode in ['all', 'aot']:
            run(name + '-aot-build', [compiler, 'compile-native', entry,
                                     '--optimize', *common, '-o', executable])
            run(name + '-aot-run', [executable, *args], marker)
            report[name + '_aot_sha256'] = hashlib.sha256(executable.read_bytes()).hexdigest()
        if options.mode in ['all', 'jit']:
            run(name + '-jit-run', [compiler, 'run', entry, *common, '--', *args], marker)
    (out / ('results-' + options.mode + '.json')).write_text(json.dumps(report, indent=2) + '\n')


if __name__ == '__main__':
    main()

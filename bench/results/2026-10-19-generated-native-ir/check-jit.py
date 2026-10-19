#!/usr/bin/env python3
"""Check arguments and child exit status through the CLI's three JIT modes."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', type=Path, required=True)
    parser.add_argument('--sdk-lib', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    options = parser.parse_args()
    compiler = options.compiler.resolve()
    repository = Path(__file__).resolve().parents[3]
    fixture = repository / 'crates/cli/tests/fixtures/jit_main_arguments.dart'
    environment = dict(os.environ, DARTFORGE_SDK_LIB=str(options.sdk_lib.resolve()),
                       DARTFORGE_MEMORY='tracing')
    environment.pop('DARTFORGE_PACKAGE_CONFIG', None)
    environment.pop('DARTFORGE_SDK_DLL', None)
    records = []
    with tempfile.TemporaryDirectory(prefix='dartforge-jit-exit-') as directory:
        failing = Path(directory) / 'exit.dart'
        failing.write_text("import 'dart:io';\nvoid main() { exit(37); }\n")
        for source, expected_exit, arguments in [
            (fixture, 0, ['--timings', '--child', '', 'é folder', '--']),
            (failing, 37, []),
        ]:
            for mode in ['run', 'reload', 'restart']:
                command = [str(compiler), 'run' if mode == 'run' else 'reload', str(source)]
                if mode != 'run':
                    command.append('--once')
                if mode == 'restart':
                    command.append('--restart')
                command += ['--', *arguments]
                child = subprocess.Popen(command, env=environment, stdout=subprocess.PIPE,
                                         stderr=subprocess.PIPE, text=True, start_new_session=True)
                try:
                    stdout, stderr = child.communicate(timeout=300)
                except subprocess.TimeoutExpired:
                    # Clean up only this test's process group, including a
                    # reload worker, before surfacing a timeout as a failure.
                    os.killpg(child.pid, signal.SIGKILL)
                    child.communicate()
                    raise
                records.append(dict(mode=mode, command=command, exit=child.returncode,
                                    stdout=stdout, stderr=stderr))
                assert child.returncode == expected_exit, records[-1]
                expected_stdout = (json.dumps(arguments, ensure_ascii=False,
                                               separators=(',', ':')) + '\n') if arguments else ''
                assert stdout.replace('\r\n', '\n') == expected_stdout, records[-1]
    report = dict(compiler=str(compiler),
                  compiler_sha256=hashlib.sha256(compiler.read_bytes()).hexdigest(),
                  cases=records, passed=len(records))
    options.out.write_text(json.dumps(report, indent=2, ensure_ascii=False) + '\n')
    print(json.dumps(dict(passed=len(records), compiler_sha256=report['compiler_sha256'])))


if __name__ == '__main__':
    main()

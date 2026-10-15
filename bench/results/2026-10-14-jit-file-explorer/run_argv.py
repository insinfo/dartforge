"""Actual UTF-8/empty/option-like main arguments through run and both reload modes."""
from pathlib import Path
import argparse, hashlib, json, os, subprocess
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parents[3]
ARCHIVE = Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--producer', type=Path, required=True)
parser.add_argument('--producer-sha256', required=True)
options = parser.parse_args()
cli = options.producer.resolve()
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
assert sha(cli) == options.producer_sha256
source = ROOT / 'crates/cli/tests/fixtures/jit_main_arguments.dart'
run_id = datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')
work = ROOT / 'target/jit-file-explorer-argv' / run_id
work.mkdir(parents=True, exist_ok=False)
evidence = ARCHIVE / 'argv-runs' / run_id
evidence.mkdir(parents=True, exist_ok=False)
env = {key: value for key, value in os.environ.items() if not key.startswith('DARTFORGE_')}
env.update(TMPDIR=str(work), DARTFORGE_SDK_LIB='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib',
           DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang',
           LLVM_SYS_221_PREFIX='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64',
           SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk')
args = ['--timings', '--child', '', 'é folder', '--']
expected = json.dumps(args, ensure_ascii=False, separators=(',', ':')) + '\n'
report = {'producer_sha256': sha(cli), 'source_sha256': sha(source), 'commands': [],
          'scope': 'Actual ordinary Dart main argument semantics and CLI supervisor/restart separator handling. No timings.'}
def save():
    (evidence / 'validation.json').write_text(json.dumps(report, indent=2) + '\n')
try:
    oracle = subprocess.run(['/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart', str(source), *args],
                            capture_output=True, text=True, env=env, timeout=60)
    (evidence / 'vm.stdout').write_text(oracle.stdout)
    (evidence / 'vm.stderr').write_text(oracle.stderr)
    assert oracle.returncode == 0 and oracle.stdout == expected and not oracle.stderr
    for mode in ['run', 'reload', 'restart']:
        command = [str(cli), 'run' if mode == 'run' else 'reload', str(source)]
        if mode == 'run':
            command = [str(cli), 'run', '--sdk', env['DARTFORGE_SDK_LIB'], str(source)]
        else:
            command.extend(['--sdk', env['DARTFORGE_SDK_LIB'], '--once'])
        if mode == 'restart':
            command.append('--restart')
        command.extend(['--', *args])
        item = {'mode': mode, 'command': command}
        report['commands'].append(item)
        save()
        print(mode, flush=True)
        result = subprocess.run(command, capture_output=True, text=True, env=env, timeout=900)
        (evidence / (mode + '.stdout')).write_text(result.stdout)
        (evidence / (mode + '.stderr')).write_text(result.stderr)
        item.update(exit=result.returncode, exact_stdout=result.stdout == expected,
                    stdout_sha256=sha(evidence / (mode + '.stdout')),
                    stderr_sha256=sha(evidence / (mode + '.stderr')))
        save()
        assert result.returncode == 0 and result.stdout == expected, mode
    report['status'] = 'passed'
    report['inputs_stable'] = sha(cli) == options.producer_sha256 and sha(source) == report['source_sha256']
    assert report['inputs_stable']
except BaseException as error:
    report.update(status='failed', error=repr(error))
    save()
    raise
save()

print(evidence / "validation.json", flush=True)

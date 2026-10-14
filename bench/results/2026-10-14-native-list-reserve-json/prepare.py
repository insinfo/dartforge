from pathlib import Path
import argparse, hashlib, json, os, subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--compiler', required=True)
parser.add_argument('--label', required=True, choices=['before', 'after'])
parser.add_argument('--phase', choices=['correctness', 'timing'], default='correctness')
parser.add_argument('--stress', action='store_true', help='Optional full-benchmark stress verification; focused reserve lifetime tests already exercise stress.')
args = parser.parse_args()
root = Path.cwd()
evidence = root / 'bench/results/2026-10-14-native-list-reserve-json'
work = root / 'target/native-provenance/list-reserve-json'
sdk = Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk')
compiler = Path(args.compiler).resolve()
def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()
if args.label == 'before':
    assert sha(compiler) == '02dd0c2ef1d5a4eeefdc9a1a76faf4d4ad0b4f4711989480bb5ec5e2087c2002'
else:
    assert sha(compiler) == '2919f3ffadb7e8d2b01b7766b82cd768482548edc0be8d58599bb902ac2d2c99'
inputs = {str(root / 'bench/performance' / name): sha(root / 'bench/performance' / name) for name in ['json.dart', 'common.dart']}
snapshot = work / 'correctness-source'
snapshot.mkdir(parents=True, exist_ok=True)
(snapshot / 'json.dart').write_bytes((root / 'bench/performance/json.dart').read_bytes())
(snapshot / 'common.dart').write_text("void measure(String name, Object? Function() kernel, {int rounds = 6}) { print('$name: ${kernel()}'); }\n")
source = snapshot / 'json.dart' if args.phase == 'correctness' else root / 'bench/performance/json.dart'
env = dict(os.environ, TMPDIR=str(root / 'target/tmp-p8-runtime'), DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang', LLVM_SYS_221_PREFIX='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64', DARTFORGE_ARC_CHECK='1', DARTFORGE_EFFECTS='check', DARTFORGE_ARC_PORT_PRUNING='1', DARTFORGE_FACTORY_RECEIVER_PROVENANCE='1')
ledger = {'compiler': str(compiler), 'compiler_sha256': sha(compiler), 'sdk': str(sdk), 'inputs': inputs, 'phase': args.phase, 'label': args.label, 'source': str(source), 'source_sha256': sha(source), 'harness_sha256': sha(source.parent / 'common.dart'), 'artifacts': {}}
if args.phase == 'correctness':
    vm = subprocess.run([str(sdk / 'bin/dart'), str(source)], capture_output=True, env=env, timeout=600)
    (evidence / 'vm.stdout').write_bytes(vm.stdout)
    (evidence / 'vm.stderr').write_bytes(vm.stderr)
    ledger['vm_exit'] = vm.returncode
    assert vm.returncode == 0
    ledger['vm_stdout_sha256'] = sha(evidence / 'vm.stdout')
for mode in ['tracing', 'legacy-arc', 'prepared-arc']:
    folder = work / args.phase / args.label / mode
    folder.mkdir(parents=True, exist_ok=True)
    artifact = folder / 'program'
    mode_env = dict(env, DARTFORGE_ARC_PREPARED='1' if mode == 'prepared-arc' else '0')
    command = [str(compiler), 'compile-native', str(source), '--sdk', str(sdk / 'lib'), '--optimize', '--memory=' + ('tracing' if mode == 'tracing' else 'arc'), '--exceptions=check', '--roots=shadow', '--stack-trace=none', '-o', str(artifact)]
    result = subprocess.run(command, capture_output=True, env=mode_env, timeout=1800)
    prefix = evidence / (args.phase + '-' + args.label + '-' + mode)
    Path(str(prefix) + '-compile.stdout').write_bytes(result.stdout)
    Path(str(prefix) + '-compile.stderr').write_bytes(result.stderr)
    row = {'command': command, 'arc_prepared': mode_env['DARTFORGE_ARC_PREPARED'], 'compile_exit': result.returncode, 'environment': {key: value for key, value in mode_env.items() if key.startswith('DARTFORGE_')}}
    if result.returncode == 0:
        row.update(artifact=str(artifact), artifact_sha256=sha(artifact), artifact_bytes=artifact.stat().st_size)
        if args.phase == 'correctness':
            row['runs'] = {}
            for stress in ([False, True] if args.stress else [False]):
                run_env = dict(mode_env)
                if stress:
                    run_env['DARTFORGE_GC_STRESS'] = '1'
                else:
                    run_env.pop('DARTFORGE_GC_STRESS', None)
                run = subprocess.run([str(artifact)], capture_output=True, env=run_env, timeout=1800)
                suffix = '-stress' if stress else '-normal'
                Path(str(prefix) + suffix + '.stdout').write_bytes(run.stdout)
                Path(str(prefix) + suffix + '.stderr').write_bytes(run.stderr)
                row['runs'][suffix] = {'exit': run.returncode, 'stdout_exact_vm': run.stdout == vm.stdout, 'stderr_empty': run.stderr == b''}
                print(args.label, mode, suffix, row['runs'][suffix], flush=True)
    else:
        print(args.label, mode, 'compile failed', result.returncode, flush=True)
    ledger['artifacts'][mode] = row
    assert all(sha(path) == digest for path, digest in inputs.items())
    (evidence / (args.phase + '-' + args.label + '.json')).write_text(json.dumps(ledger, indent=2) + '\n')

#!/usr/bin/env python3
"""Measure alternating baseline/deferred allocation frames without speed assertions."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
import statistics
import subprocess

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'bench/comparison/gc_frame.dart'
PACKAGE = ROOT / 'packages/dartforge/.dart_tool/package_config.json'
SSD_DART = Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart')
SSD_CLANG = Path('/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang')
CLEAN_ENVIRONMENT = (
    'DARTFORGE_GC_STRESS', 'DARTFORGE_GC_SCHEDULE', 'DARTFORGE_GC_OFF',
    'DARTFORGE_GC_VERIFY', 'DARTFORGE_GC_POISON', 'DARTFORGE_GC_TRACE',
    'DARTFORGE_GC_MEMORY', 'DARTFORGE_GC_YOUNG_KB', 'DARTFORGE_GC_GROWTH',
    'DARTFORGE_ARC_PREPARED', 'DARTFORGE_ARC_PREPARE',
)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def executable(value, label):
    candidate = Path(value).expanduser()
    if not candidate.is_file():
        found = shutil.which(str(value))
        if found:
            candidate = Path(found)
    candidate = candidate.resolve()
    if not candidate.is_file() or not os.access(candidate, os.X_OK):
        raise RuntimeError(f'{label} executable not found: {value}')
    return candidate


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', type=Path, default=ROOT / 'target/p4/release/dartforge')
    parser.add_argument('--dart', default=str(SSD_DART) if SSD_DART.is_file() else 'dart',
                        help='Official Dart executable used as the checksum oracle and SDK source')
    parser.add_argument('--clang', default=os.environ.get('DARTFORGE_CLANG') or
                        (str(SSD_CLANG) if SSD_CLANG.is_file() else 'clang'),
                        help='Clang executable; its directory is also prepended to PATH for linkers')
    parser.add_argument('--sdk', type=Path, help='Native compiler SDK lib directory; default derives from --dart')
    parser.add_argument('--output', type=Path, help='Empty output directory; default is a new timestamped SSD workspace directory')
    parser.add_argument('--pairs', type=int, default=3, help='Alternating pairs, 3..12 (default: 3)')
    parser.add_argument('--frames', type=int, default=20, help='Measured frames per process, 10..30')
    parser.add_argument('--items', type=int, default=768, help='Frame items, 64..2048; each owns a map and 4096-byte buffer')
    args = parser.parse_args()
    if not 3 <= args.pairs <= 12 or not 10 <= args.frames <= 30 or not 64 <= args.items <= 2048:
        parser.error('Expected 3..12 pairs, 10..30 frames and 64..2048 items')
    compiler = executable(args.compiler, 'DartForge')
    dart = executable(args.dart, 'Dart')
    clang = executable(args.clang, 'Clang')
    sdk = (args.sdk or dart.parent.parent / 'lib').resolve()
    if not sdk.is_dir() or not PACKAGE.is_file():
        raise RuntimeError('SDK lib directory or package_config.json missing; resolve packages/dartforge dependencies first')
    stamp = datetime.now(timezone.utc).strftime('%Y%m%d-%H%M%S-%f')
    output = (args.output or ROOT / 'target/gc-frame-measurement' / stamp).resolve()
    if output.exists() and (not output.is_dir() or any(output.iterdir())):
        raise RuntimeError(f'Refusing to overwrite existing evidence: {output}')
    output.mkdir(parents=True, exist_ok=True)
    binary = output / 'gc-frame.tracing'
    env = dict(os.environ, TMPDIR=str(output), DARTFORGE_EFFECTS='check',
               DARTFORGE_CLANG=str(clang), PATH=str(clang.parent) + os.pathsep + os.environ.get('PATH', ''))
    removed = {key: env.pop(key) for key in CLEAN_ENVIRONMENT if key in env}
    inputs = [SOURCE, PACKAGE, Path(__file__).resolve(), ROOT / 'packages/dartforge/lib/src/gc_policy.dart',
              ROOT / 'crates/runtime/src/heap.rs', ROOT / 'crates/runtime/src/gc_policy.rs',
              ROOT / 'crates/runtime/src/events.rs']
    ledger = {
        'schema': 2, 'compiler_sha256': sha(compiler),
        'inputs': {str(p.relative_to(ROOT)): sha(p) for p in inputs},
        'normal_only': True, 'same_binary': True, 'performance_guarantee': False,
        'workload_arguments': {'pairs': args.pairs, 'frames': args.frames, 'items': args.items},
        'tool_paths': {'compiler': str(compiler), 'dart': str(dart), 'clang': str(clang), 'sdk_lib': str(sdk)},
        'tool_hashes': {'dart_sha256': sha(dart), 'clang_sha256': sha(clang)},
        'environment': {key: value for key, value in env.items()
                        if key.startswith('DARTFORGE_') or key in ('TMPDIR', 'CARGO_HOME', 'RUSTUP_HOME', 'PUB_CACHE')},
        'removed_environment_keys': sorted(removed),
        'limitations': [
            'Short synthetic object/map/typed-buffer allocation probe, not production HTTP or UI latency.',
            'Normal runtime effect checking remains enabled in both modes.',
            'Idle elapsed time includes the precreated fixed-delay timer wait.',
            'Process order alternates baseline/deferred; machine background load is not isolated.',
            'No RSS measurement or speedup assertion; deferral can trade allocation throughput for scheduling control.',
        ],
        'runs': [], 'commands': [], 'inputs_stable': False,
    }

    def save():
        (output / 'ledger.json').write_text(json.dumps(ledger, indent=2) + '\n')

    def run(command, label, allow_stderr=False):
        command = list(map(str, command))
        result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, timeout=300)
        (output / (label + '.stdout')).write_bytes(result.stdout)
        (output / (label + '.stderr')).write_bytes(result.stderr)
        ledger['commands'].append({'label': label, 'command': command, 'exit_code': result.returncode})
        save()
        if result.returncode or (not allow_stderr and result.stderr):
            raise RuntimeError(f'{label}: execution failed; inspect raw outputs in {output}')
        return result

    for name, tool in [('dart', dart), ('clang', clang)]:
        result = run([tool, '--version'], name + '-version', allow_stderr=True)
        ledger.setdefault('tool_versions', {})[name] = (result.stdout + result.stderr).decode().strip()
    run([compiler, 'compile-native', SOURCE, '--sdk', sdk, '--packages', PACKAGE,
         '--optimize', '--exceptions=tables', '--roots=shadow', '--stack-trace=none',
         '--dartforge-contracts=verify', '--memory=tracing', '-o', binary], 'build', allow_stderr=True)
    ledger['binary_sha256'] = sha(binary)
    manifest = Path(str(binary) + '.contracts.json')
    ledger['contract_manifest_sha256'] = sha(manifest)
    report = json.loads(manifest.read_text())
    if report['certified'] is not False or report['mode'] != 'verify':
        raise RuntimeError('Unexpected contract certification or policy')
    workload = [f'--frames={args.frames}', f'--items={args.items}']
    vm = json.loads(run([dart, '--packages=' + str(PACKAGE), SOURCE, *workload], 'vm-baseline').stdout)
    expected = [sample['checksum'] for sample in vm['samples']]
    checksum = vm['checksum']
    if vm['frames'] != args.frames or vm['items_per_frame'] != args.items or vm['deferred']:
        raise RuntimeError('Official VM workload configuration mismatch')
    ledger['oracle'] = {'kind': 'official_dart_vm_baseline', 'checksum': checksum, 'frame_checksums': expected}

    def assert_stable():
        return sha(compiler) == ledger['compiler_sha256'] and sha(binary) == ledger['binary_sha256'] and all(
            sha(ROOT / name) == value for name, value in ledger['inputs'].items()) and (
            sha(dart) == ledger['tool_hashes']['dart_sha256'] and sha(clang) == ledger['tool_hashes']['clang_sha256'])

    for repetition in range(args.pairs):
        for deferred in (False, True):
            if not assert_stable():
                raise RuntimeError('Compiler, binary, tools or source changed before measurement')
            label = f'{repetition + 1}-' + ('defer' if deferred else 'baseline')
            data = json.loads(run([binary, *workload, *(['--defer'] if deferred else [])], label).stdout)
            if not assert_stable():
                raise RuntimeError('Compiler, binary, tools or source changed during measurement')
            if (data['checksum'] != checksum or [sample['checksum'] for sample in data['samples']] != expected
                    or not data['gc_supported'] or data['deferred'] != deferred
                    or data['frames'] != args.frames or data['items_per_frame'] != args.items):
                raise RuntimeError(label + ': workload/checksum mismatch')
            for sample in data['samples']:
                if (sample['deferral_accepted'] != deferred or
                    sample['collections'] != sample['after_collections'] - sample['before_collections'] or
                    sample['idle_collections'] != sample['after_idle_collections'] - sample['after_collections'] or
                    sample['idle_collections'] < 1):
                    raise RuntimeError(label + ': invalid deferral status or observed GC counters')
            durations = [sample['elapsed_micros'] for sample in data['samples']]
            data.update(frame_median_micros=statistics.median(durations), frame_max_micros=max(durations),
                        label=label, binary_sha256=ledger['binary_sha256'])
            ledger['runs'].append(data)
            save()
            print(label, 'checksum', data['checksum'], 'frame_us', data['timed_micros'],
                  'frame_gc', data['timed_collections'], 'idle_gc', data['idle_collections'], flush=True)
    ledger['inputs_stable'] = assert_stable()
    if not ledger['inputs_stable']:
        raise RuntimeError('Inputs changed during measurement')
    save()
    print('Raw ledger:', output / 'ledger.json')


if __name__ == '__main__':
    try:
        main()
    except (OSError, RuntimeError, ValueError, subprocess.TimeoutExpired) as error:
        raise SystemExit(f'error: {error}') from error

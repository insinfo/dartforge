"""Prepare matched color executables, then measure consumed output in a quiet window."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import re
import statistics
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parents[4]
ARCHIVE = Path(__file__).resolve().parent
SDK = Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk')
CLANG = Path('/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang')
SOURCE = ROOT / 'bench/comparison/color_numeric_performance.dart'
FORMAT = Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui/lib/src/graphics/image/codecs/color/format.dart')
DEFAULT_PACKAGES = Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui/examples/file_explorer/.dart_tool/package_config.json')


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def clean_environment(work):
    env = {key: value for key, value in os.environ.items() if not key.startswith('DARTFORGE_')}
    env.update(TMPDIR=str(work), DARTFORGE_CLANG=str(CLANG),
               LLVM_SYS_221_PREFIX=str(CLANG.parent.parent), DARTFORGE_SDK_LIB=str(SDK / 'lib'))
    return env


def frozen_inputs(producer, packages):
    paths = {SOURCE, FORMAT, producer, packages, SDK / 'bin/dart', CLANG, Path(shutil.which('rustc')).resolve()}
    paths.update((SDK / 'lib').rglob('*.json'))
    paths.update((SDK / 'lib').rglob('*.dart'))
    paths.update((ROOT / 'native_sdk').rglob('*.dart'))
    return {str(path): sha(path) for path in sorted(paths)}


def verify_inputs(inputs):
    differences = [path for path, digest in inputs.items() if not Path(path).is_file() or sha(path) != digest]
    if differences:
        raise RuntimeError('Frozen inputs changed: ' + ', '.join(differences))


def inspect_ir(path):
    counts = {'selector_calls': 0, 'indirect_selector_calls': 0, 'runtime_dynamic_operator_calls': 0,
              'primitive_class_guards': 0, 'box_int_calls': 0, 'box_double_calls': 0,
              'exception_clear_calls': 0, 'conditional_branches': 0}
    functions = {}
    current = None
    with path.open() as stream:
        for line in stream:
            if line.startswith('define '):
                current = line.strip()
                functions[current] = dict.fromkeys(counts, 0)
            if current is None:
                continue
            flags = {
                'selector_calls': 'call ptr @df.selector(' in line,
                'indirect_selector_calls': bool(re.search(r'call [^\n]* %sf\d+\(', line)),
                'runtime_dynamic_operator_calls': '@dartforge_dyn_op(' in line and 'call ' in line,
                'primitive_class_guards': 'call i64 @df.class(' in line,
                'box_int_calls': 'call i64 @df.box_int(' in line,
                'box_double_calls': 'call i64 @df.box_double(' in line,
                'exception_clear_calls': 'call void @dartforge_exception_clear(' in line,
                'conditional_branches': bool(re.search(r'\bbr i1 ', line)),
            }
            for key, present in flags.items():
                counts[key] += int(present)
                functions[current][key] += int(present)
            if line.startswith('}'):
                current = None
    focused = {header: value for header, value in functions.items()
               if 'convertFormatValue' in header or 'convertPixels' in header}
    return {'path': str(path), 'sha256': sha(path), 'bytes': path.stat().st_size,
            'all_defined_bodies': counts, 'focused_functions': focused,
            'scope': 'Pre-LTO static IR sites, including guarded slow branches; these are not executed call counts.'}


def compile_artifacts(args, producer, work):
    work.mkdir(parents=True, exist_ok=False)
    for label in ['official', 'off', 'constants', 'clamp', 'combined']:
        (work / label).mkdir()
    inputs = frozen_inputs(producer, args.packages)
    report = {'complete': False, 'producer': str(producer), 'producer_sha256': args.sha256,
              'source': str(SOURCE), 'packages': str(args.packages), 'work': str(work), 'inputs': inputs,
              'rustc_version': subprocess.check_output(['rustc', '--version'], text=True).strip(),
              'artifacts': {}, 'commands': {}, 'scope': 'Compilation and static IR inspection only; no executions or timings.'}
    manifest = ARCHIVE / 'prepared.json'
    if manifest.exists():
        raise RuntimeError('prepared.json already exists; preserve frozen evidence in a new archive directory.')
    write_json(manifest, report)
    env = clean_environment(work)
    for label in ['official', 'off', 'constants', 'clamp', 'combined']:
        binary = work / label / 'program'
        effective = env.copy()
        if label == 'official':
            command = [str(SDK / 'bin/dart'), 'compile', 'exe', '--packages=' + str(args.packages),
                       str(SOURCE), '-o', str(binary)]
        else:
            effective.update(DARTFORGE_KEEP_IR='1', DARTFORGE_LINK_MAP='1')
            effective['DARTFORGE_CONSTANT_SPECIALIZATION'] = '1' if label in ['constants', 'combined'] else '0'
            effective['DARTFORGE_PRIMITIVE_CLAMP'] = '1' if label in ['clamp', 'combined'] else '0'
            command = [str(producer), 'compile-native', str(SOURCE), '--sdk', str(SDK / 'lib'),
                       '--packages', str(args.packages), '--optimize', '--memory=tracing',
                       '--exceptions=check', '--roots=shadow', '--stack-trace=none', '-o', str(binary)]
        stdout = ARCHIVE / (label + '-compile.stdout')
        stderr = ARCHIVE / (label + '-compile.stderr')
        with stdout.open('wb') as output, stderr.open('wb') as errors:
            result = subprocess.run(command, env=effective, stdout=output, stderr=errors, timeout=args.compile_timeout)
        report['commands'][label] = {'argv': command, 'exit_code': result.returncode,
            'effective_environment': {key: value for key, value in effective.items()
                                      if key.startswith('DARTFORGE_') or key in ['TMPDIR', 'LLVM_SYS_221_PREFIX']},
            'stdout_sha256': sha(stdout), 'stderr_sha256': sha(stderr)}
        write_json(manifest, report)
        if result.returncode != 0:
            raise RuntimeError(label + ' compilation failed; raw failure retained.')
        artifact = {'path': str(binary), 'sha256': sha(binary), 'bytes': binary.stat().st_size}
        if label != 'official':
            ir = work / label / '.df_tmp/program.ll'
            artifact['ir'] = inspect_ir(ir)
            write_json(ARCHIVE / (label + '-ir.json'), artifact['ir'])
        report['artifacts'][label] = artifact
        write_json(manifest, report)
        print(label, 'compiled', artifact['bytes'], flush=True)
    verify_inputs(inputs)
    report.update(complete=True, input_hashes_unchanged=True)
    write_json(manifest, report)


def parse_samples(stdout):
    lines = stdout.splitlines()
    parsed = [re.fullmatch(r'(-?\d+):(\d+)', line) for line in lines]
    if len(parsed) != 5 or any(match is None for match in parsed):
        raise RuntimeError('Expected exactly five consumed checksum:microseconds samples: ' + stdout)
    return [int(match[1]) for match in parsed], [int(match[2]) for match in parsed]


def measure_artifacts(args, producer, work):
    manifest = ARCHIVE / 'prepared.json'
    prepared = json.loads(manifest.read_text())
    if not prepared['complete'] or prepared['producer'] != str(producer) or prepared['producer_sha256'] != args.sha256:
        raise RuntimeError('Measurement requires completed preparation for this exact frozen producer.')
    verify_inputs(prepared['inputs'])
    if prepared['work'] != str(work):
        raise RuntimeError('Measurement must use the prepared SSD work directory.')
    if subprocess.check_output(['rustc', '--version'], text=True).strip() != prepared['rustc_version']:
        raise RuntimeError('Rust toolchain version changed since compilation.')
    for artifact in prepared['artifacts'].values():
        if sha(artifact['path']) != artifact['sha256']:
            raise RuntimeError('Frozen artifact changed: ' + artifact['path'])
    output = ARCHIVE / 'measurement'
    output.mkdir(exist_ok=False)
    env = clean_environment(work)
    orders = [['off', 'constants', 'clamp', 'combined', 'official'], ['official', 'combined', 'clamp', 'constants', 'off'], ['constants', 'clamp', 'combined', 'official', 'off'], ['combined', 'official', 'off', 'clamp', 'constants'], ['clamp', 'combined', 'official', 'off', 'constants']]
    ledger = {'complete': False, 'iterations': args.iterations, 'orders': orders, 'rows': [],
              'prepared_sha256': sha(manifest), 'artifacts': prepared['artifacts'],
              'scope': 'Five alternating fresh processes per profile. First of five kernel samples discarded; process wall/RSS includes all five. No other builds or timing workloads should run during this command.'}
    destination = ARCHIVE / 'measurement.json'
    write_json(destination, ledger)
    command = [str(SDK / 'bin/dart'), '--packages=' + prepared['packages'], str(SOURCE), str(args.iterations)]
    oracle = subprocess.run(command, env=env, capture_output=True, text=True, timeout=args.timeout)
    (output / 'vm.stdout').write_text(oracle.stdout)
    (output / 'vm.stderr').write_text(oracle.stderr)
    ledger['oracle'] = {'command': command, 'exit_code': oracle.returncode,
                        'stdout_sha256': sha(output / 'vm.stdout'), 'stderr_sha256': sha(output / 'vm.stderr')}
    write_json(destination, ledger)
    if oracle.returncode != 0:
        raise RuntimeError('VM checksum oracle failed.')
    checksums, _ = parse_samples(oracle.stdout)
    if len(set(checksums)) != 1:
        raise RuntimeError('VM consumed checksums vary.')
    expected = checksums[0]
    ledger['expected_checksum'] = expected
    for round_index, order in enumerate(orders):
        for label in order:
            command = ['/usr/bin/time', '-l', prepared['artifacts'][label]['path'], str(args.iterations)]
            start = time.perf_counter_ns()
            result = subprocess.run(command, env=env, capture_output=True, text=True, timeout=args.timeout)
            wall_ms = (time.perf_counter_ns() - start) / 1e6
            stdout = output / f'{round_index}-{label}.stdout'
            stderr = output / f'{round_index}-{label}.stderr'
            stdout.write_text(result.stdout)
            stderr.write_text(result.stderr)
            row = {'round': round_index, 'profile': label, 'command': command, 'exit_code': result.returncode,
                   'wall_ms': wall_ms, 'stdout_sha256': sha(stdout), 'stderr_sha256': sha(stderr)}
            ledger['rows'].append(row)
            write_json(destination, ledger)
            if result.returncode != 0:
                raise RuntimeError(label + ' process failed; raw output retained.')
            checksums, samples = parse_samples(result.stdout)
            row.update(checksums=checksums, kernel_microseconds=samples,
                       discarded_first_sample=True, consumed_checksum_exact=all(value == expected for value in checksums))
            rss = re.search(r'(\d+)\s+maximum resident set size', result.stderr)
            if rss is None or not row['consumed_checksum_exact']:
                write_json(destination, ledger)
                raise RuntimeError('Missing RSS or incorrect consumed checksum; no successful measurement claim.')
            row['max_rss_bytes'] = int(rss[1])
            row['retained_kernel_median_microseconds'] = statistics.median(samples[1:])
            write_json(destination, ledger)
            print(round_index, label, row['retained_kernel_median_microseconds'], flush=True)
    ledger['medians'] = {}
    for label in ['official', 'off', 'constants', 'clamp', 'combined']:
        rows = [row for row in ledger['rows'] if row['profile'] == label]
        ledger['medians'][label] = {
            'kernel_microseconds': statistics.median(value for row in rows for value in row['kernel_microseconds'][1:]),
            'process_kernel_medians_microseconds': [row['retained_kernel_median_microseconds'] for row in rows],
            'wall_ms': statistics.median(row['wall_ms'] for row in rows),
            'max_rss_bytes': statistics.median(row['max_rss_bytes'] for row in rows),
        }
    ledger['paired_combined_over_off_kernel_ratios'] = []
    for index in range(len(orders)):
        rows = {row['profile']: row for row in ledger['rows'] if row['round'] == index}
        denominator = rows['off']['retained_kernel_median_microseconds']
        ledger['paired_combined_over_off_kernel_ratios'].append(
            rows['combined']['retained_kernel_median_microseconds'] / denominator if denominator else None)
    verify_inputs(prepared['inputs'])
    for artifact in prepared['artifacts'].values():
        if sha(artifact['path']) != artifact['sha256']:
            raise RuntimeError('Artifact changed during measurement.')
    ledger.update(complete=True, inputs_unchanged=True, artifacts_unchanged=True)
    write_json(destination, ledger)
    print(json.dumps(ledger['medians'], indent=2), flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument('--compile', action='store_true')
    action.add_argument('--measure', action='store_true')
    parser.add_argument('--producer', type=Path, required=True)
    parser.add_argument('--sha256', required=True)
    parser.add_argument('--packages', type=Path, default=DEFAULT_PACKAGES)
    parser.add_argument('--work', type=Path)
    parser.add_argument('--iterations', type=int, default=1000000)
    parser.add_argument('--timeout', type=int, default=180)
    parser.add_argument('--compile-timeout', type=int, default=600)
    args = parser.parse_args()
    producer = args.producer.resolve()
    args.packages = args.packages.resolve()
    if sha(producer) != args.sha256:
        raise RuntimeError('Producer SHA256 differs from the explicitly frozen binary.')
    if args.iterations <= 0:
        raise RuntimeError('Iterations must be positive.')
    work = (args.work or ROOT / 'target' / ('primitive-clamp-performance-' + args.sha256[:12])).resolve()
    if not str(work).startswith('/Volumes/ssd_dock/'):
        raise RuntimeError('Build artifacts and temporary files must stay on the SSD.')
    if args.compile:
        compile_artifacts(args, producer, work)
    else:
        measure_artifacts(args, producer, work)


if __name__ == '__main__':
    main()

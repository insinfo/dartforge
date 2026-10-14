#!/usr/bin/env python3
"""Validate artifact-bound imported return facts with an ordinary copied SDK factory.

The official oracle defines the same fresh-list factory locally because the official
VM uses its precompiled platform and does not expose a newly added SDK function.
This validates transport/correctness; it is not a stock-SDK performance comparison.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[3]
STOCK_LIB = Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib')
DART = STOCK_LIB.parent / 'bin/dart'
FACTORY = '\nList<Object?> dartforgeFreshList() => <Object?>[];\n'
MODIFIED_FACTORY = '\nList<Object?> dartforgeFreshList() => const <Object?>["changed"];\n'
APP = """import 'dart:convert';
class OtherList {
  int calls = 0;
  @override
  dynamic noSuchMethod(Invocation invocation) {
    calls++;
    return 7;
  }
}
@pragma('vm:never-inline')
int unknownLength(dynamic value) => value.length;
void main() {
  dynamic list = dartforgeFreshList();
  try { list.add('ssd'); } on UnsupportedError { print('immutable'); }
  print(list);
  print(list.length);
  final other = OtherList();
  print(unknownLength(other));
  print(other.calls);
}
"""

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def execute(command, directory, name, environment=None):
    completed = subprocess.run(command, cwd=ROOT, env=environment, capture_output=True, timeout=900)
    (directory / f'{name}.stdout').write_bytes(completed.stdout)
    (directory / f'{name}.stderr').write_bytes(completed.stderr)
    record = {'command': [str(p) for p in command], 'exit_code': completed.returncode,
              'stdout_sha256': hashlib.sha256(completed.stdout).hexdigest(),
              'stderr_sha256': hashlib.sha256(completed.stderr).hexdigest()}
    (directory / f'{name}.json').write_text(json.dumps(record, indent=2) + '\n')
    if completed.returncode:
        raise RuntimeError(f'{name} failed; raw stdout/stderr retained')
    return completed.stdout


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--out', type=Path, default=ROOT / 'target/sdk-allocation-copied-cli')
    parser.add_argument('--producer', type=Path)
    parser.add_argument('--producer-sha')
    parser.add_argument('--prepare-only', action='store_true')
    arguments = parser.parse_args()
    output = arguments.out.resolve()
    output.mkdir(parents=True, exist_ok=True)
    copied = output / 'sdk/lib'
    if copied.exists():
        raise RuntimeError('Use a new output directory; existing copied SDK evidence is preserved')
    shutil.copytree(STOCK_LIB, copied, symlinks=False)
    convert = copied / 'convert/convert.dart'
    original = convert.read_text()
    convert.write_text(original + FACTORY)
    entry = output / 'main.dart'
    entry.write_text(APP)
    oracle = output / 'oracle.dart'
    oracle.write_text(APP + FACTORY)
    prepared = {'copied_sdk': str(copied), 'entry': str(entry), 'entry_sha256': sha(entry),
                'selected_factory_unit': str(convert), 'factory_unit_sha256': sha(convert),
                'stock_factory_unit_sha256': sha(STOCK_LIB / 'convert/convert.dart'),
                'official_oracle_scope': 'Same ordinary source list factory defined locally; official platform cannot expose injected SDK member.',
                'no_stock_sdk_performance_claim': True}
    (output / 'prepared.json').write_text(json.dumps(prepared, indent=2) + '\n')
    if arguments.prepare_only:
        print(json.dumps(prepared, indent=2))
        return
    if not arguments.producer or not arguments.producer_sha or sha(arguments.producer) != arguments.producer_sha:
        raise RuntimeError('Exact frozen producer and SHA required')
    base = os.environ.copy()
    for key in list(base):
        if key.startswith('DARTFORGE_'):
            base.pop(key)
    base.update({'DARTFORGE_CLANG': '/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang',
                 'DARTFORGE_KEEP_IR': '1', 'DARTFORGE_SDK_LIB': str(copied),
                 'DARTFORGE_NATIVE_CACHE': str(output / 'native-cache'), 'TMPDIR': str(output)})
    results = []
    for changed in [False, True]:
        if changed:
            convert.write_text(original + MODIFIED_FACTORY)
            oracle.write_text(APP + MODIFIED_FACTORY)
        expected = execute([DART, oracle], output, f'oracle-{changed}')
        for disabled in [True, False]:
            environment = base.copy()
            if disabled:
                environment['DARTFORGE_NO_SDK_ALLOCATION_PROOFS'] = '1'
            # Cold and warm consume the same producer, selected SDK source and policies.
            for warmth in ['cold', 'warm']:
                name = f'{"modified" if changed else "fresh"}-{"off" if disabled else "on"}-{warmth}'
                directory = output / name
                directory.mkdir()
                binary = directory / 'program'
                environment['TMPDIR'] = str(directory)
                execute([arguments.producer, 'compile-native', entry, '--sdk', copied, '--optimize',
                         '--memory=tracing', '--exceptions=check', '--roots=shadow', '--stack-trace=none',
                         '-o', binary], directory, 'compile', environment)
                actual = execute([binary], directory, 'execute', environment)
                assert actual == expected, (name, actual, expected)
                stress = environment.copy()
                stress['DARTFORGE_GC_STRESS'] = '1'
                assert execute([binary], directory, 'gc-stress', stress) == expected
                ir = directory / '.df_tmp/program.ll'
                text = ir.read_text()
                symbol = 'df.dart$3aconvert..dartforgeFreshList'
                assert symbol in text, f'{name}: selected SDK factory call missing'
                # Preserve local call-site regions and complete proof manifests for review.
                lines = text.splitlines()
                main_start = next(i for i, line in enumerate(lines) if line.startswith('define ') and '@dart_main(' in line)
                main_end = next(i for i in range(main_start + 1, len(lines)) if lines[i] == '}')
                main_ir = '\n'.join(lines[main_start:main_end + 1])
                (directory / 'main.ll').write_text(main_ir + '\n')
                imported_list_fact = '; df.selector-exact 10' in main_ir
                assert imported_list_fact == (not disabled and not changed), (name, 'imported downstream list fact mismatch', imported_list_fact)
                indices = [i for i, line in enumerate(lines) if symbol in line and 'call' in line]
                (directory / 'factory-ir-region.txt').write_text('\n\n'.join('\n'.join(lines[max(0, i - 4):i + 70]) for i in indices) + '\n')
                results.append({'case': name, 'program_bytes': binary.stat().st_size, 'program_sha256': sha(binary),
                                'oracle_match': True, 'gc_stress_oracle_match': True, 'factory_call_regions': len(indices), 'imported_downstream_list_fact': imported_list_fact})
    proof_files = []
    for path in sorted((output / 'native-cache/sdk').glob('*/allocation-returns.json')):
        value = json.loads(path.read_text())
        exports = [export for export in value['exports'] if export['symbol'].endswith('..dartforgeFreshList')]
        proof_files.append({'path': str(path), 'sha256': sha(path), 'cache_key': value['cache_key'], 'factory_exports': exports})
    assert any(record['factory_exports'] and record['factory_exports'][0]['classes'] == [10] for record in proof_files), 'fresh copied SDK factory certificate missing'
    assert any(not record['factory_exports'] for record in proof_files), 'modified constant-cache factory must remain uncertified'
    validation = {'producer': str(arguments.producer), 'producer_sha256': sha(arguments.producer),
                  'cases': results, 'proof_manifests': proof_files, 'scope': prepared['official_oracle_scope'],
                  'timing_claim': False, 'stock_sdk_size_gain_claim': False}
    (output / 'validation.json').write_text(json.dumps(validation, indent=2) + '\n')
    print(json.dumps(validation, indent=2))


if __name__ == '__main__':
    main()

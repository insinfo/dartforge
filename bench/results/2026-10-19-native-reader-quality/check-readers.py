#!/usr/bin/env python3
"""Compare bounded-reader regressions with the recorded baseline, using rustc.

The two std-only modules are compiled verbatim with a target-selection stub;
this is not a test of object generation, the native runtime, or a foreign OS.
Baseline property tests with hostile allocation sizes are deliberately excluded:
only the five bounded, deterministic regressions run on the old implementation.
"""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[3]
OUTPUT = Path(__file__).resolve().parent
BASELINE = '3e1aa0e5'
SELECTED = {
    'gcmap': {
        'binary_readers_reject_overflowing_offsets',
        'varints_reject_values_outside_u64',
        'compact_maps_reject_frame_and_return_overflow',
        'llvm_stack_maps_bound_declared_counts_before_allocation',
    },
    'compact_stack_trace': {'compact_trace_rejects_inconsistent_size_and_record_index'},
}


def regression_tests(source, names):
    tests = []
    # Selected functions precede the next test; their bodies are retained verbatim.
    for block in source.split('    #[test]\n')[1:]:
        name = re.search(r'fn (\w+)\(', block).group(1)
        if name in names:
            tests.append('    #[test]\n' + block)
    assert len(tests) == len(names)
    return '\n#[cfg(test)]\nmod reader_regressions {\n    use super::*;\n' + ''.join(tests) + '}\n'


def main():
    metadata = {'baseline': BASELINE, 'checks': [], 'sources': {}}
    current = {}
    for module in SELECTED:
        path = ROOT / 'crates/emit_native/src' / (module + '.rs')
        current[module] = path.read_text()
        metadata['sources'][str(path.relative_to(ROOT))] = hashlib.sha256(path.read_bytes()).hexdigest()
    for revision in ('before', 'after', 'after-optimized'):
        with tempfile.TemporaryDirectory(prefix='dartforge-reader-check-') as temporary:
            directory = Path(temporary)
            for module, names in SELECTED.items():
                source = current[module]
                if revision == 'before':
                    source = subprocess.check_output(
                        ['git', 'show', f'{BASELINE}:crates/emit_native/src/{module}.rs'],
                        cwd=ROOT, text=True)
                    source += regression_tests(current[module], names)
                (directory / (module + '.rs')).write_text(source)
            (directory / 'harness.rs').write_text('''#![allow(dead_code)]
mod target {
    #[derive(PartialEq)]
    pub enum Os { Linux, Windows, MacOs }
    pub fn target_os() -> Os { Os::MacOs }
}
mod gcmap;
mod compact_stack_trace;
''')
            runner = directory / 'reader-tests'
            flags = ['-O'] if revision == 'after-optimized' else []
            built = subprocess.run(['rustc', '--edition=2024', '--test', *flags,
                                    str(directory / 'harness.rs'), '-o', str(runner)],
                                   capture_output=True, text=True)
            assert built.returncode == 0, built.stderr
            command = [str(runner)]
            if revision == 'before':
                command.append('reader_regressions')
            result = subprocess.run(command, capture_output=True, text=True, timeout=30)
            log = result.stdout + result.stderr
            (OUTPUT / f'readers-{revision}.txt').write_text(log.rstrip() + '\n')
            expected = 101 if revision == 'before' else 0
            assert result.returncode == expected, log
            summary = re.search(r'test result: .*', log).group(0)
            metadata['checks'].append({'revision': revision, 'exit_code': result.returncode,
                                       'summary': summary})
    (OUTPUT / 'reader-checks.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(json.dumps(metadata, indent=2))


if __name__ == '__main__':
    main()

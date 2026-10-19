"""Recheck analyzer_rules.rs cases against an actual Dart 3.6.2 invocation."""
from collections import Counter
from pathlib import Path
import hashlib
import json
import os
import subprocess
import tempfile

out = Path(__file__).resolve().parent
source = Path('crates/parity/tests/analyzer_rules.rs').read_text()
start = source.index('const CASES:')
constant = source[start:source.index('\n#[test]', start)]
extract = 'use std::{fs,path::Path};\n' + constant + r'''
fn main() {
    let dir = std::env::args().nth(1).unwrap();
    let root = Path::new(&dir);
    let mut expected = String::new();
    for (name, text, records) in CASES {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
        for (code, line, column, length) in *records {
            expected.push_str(&format!("{name}\t{code}\t{line}\t{column}\t{length}\n"));
        }
    }
    fs::write(root.join("expected.tsv"), expected).unwrap();
    println!("{}", CASES.len());
}
'''
rs = Path('target/diagnostics-oracle-extract.rs')
rs.write_text(extract)
exe = Path('target/diagnostics-oracle-extract').resolve()
subprocess.run(['rustc', '--edition=2024', str(rs), '-o', str(exe)], check=True)
dart = os.environ.get('DARTFORGE_DART', 'dart')
version = subprocess.check_output([dart, '--version'], text=True, stderr=subprocess.STDOUT).strip()
assert 'version: 3.6.2 ' in version, version
with tempfile.TemporaryDirectory(prefix='diagnostics-oracle-', dir='target') as directory:
    root = Path(directory).resolve()
    cases = int(subprocess.check_output([str(exe), str(root)], text=True))
    (root/'pubspec.yaml').write_text('name: regras\npublish_to: none\nenvironment:\n  sdk: ^3.6.0\n')
    (root/'.dart_tool').mkdir()
    (root/'.dart_tool/package_config.json').write_text(json.dumps(dict(
        configVersion=2, packages=[dict(name='regras', rootUri='../', packageUri='lib/', languageVersion='3.6')]
    )))
    expected = Counter(tuple(row.split('\t')) for row in (root/'expected.tsv').read_text().splitlines())
    assert cases > 0 and expected
    result = subprocess.run([dart, 'analyze', '--format', 'machine', str(root)], capture_output=True, text=True)
    assert 0 <= result.returncode <= 3, result.stderr
    text = result.stdout + result.stderr
    (out/'fresh-oracle.txt').write_text('\n'.join(line.rstrip() for line in text.splitlines())+'\n')
    actual = Counter()
    for line in text.splitlines():
        fields = line.split('|', 7)
        assert len(fields) == 8, line
        _, _, code, file, row, column, length, _ = fields
        actual[(str(Path(file).relative_to(root)), code.lower(), row, column, length)] += 1
    missing = list((expected-actual).elements())
    extra = list((actual-expected).elements())
    sources = {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(root.rglob('*.dart'))}
    assert len(sources) == cases
    record = dict(dart_version=version, dart_exit_code=result.returncode, cases=cases,
                  expected=sum(expected.values()), actual=sum(actual.values()),
                  compared_fields=['file', 'code', 'line', 'column', 'length'],
                  missing=missing, extra=extra, sources_sha256=sources)
    (out/'oracle-comparison.json').write_text(json.dumps(record, indent=2)+'\n')
    assert not missing and not extra, record
    print(f'{cases} cases; {sum(actual.values())} diagnostics match code and position.')

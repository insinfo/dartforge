"""Exercise real probe/recorder binaries using temporary sources and negative references."""
from pathlib import Path
import hashlib
import json
import os
import subprocess
import tempfile

out = Path(__file__).resolve().parent
suffix = '.exe' if os.name == 'nt' else ''
probe = Path('target/debug/examples/probe_files' + suffix).resolve()
recorder = Path('target/debug/dartforge-parity' + suffix).resolve()
sdk = Path(os.environ['DARTFORGE_ORACLE_3_6']).resolve()
environment = os.environ.copy()
environment.setdefault('DARTFORGE_SDK_LIB', str(sdk/'lib'))
results = []

def invoke(label, args, expected, extra=None, main=False):
    env = environment.copy()
    env.update(extra or {})
    result = subprocess.run([str(recorder if main else probe), *args], env=env,
                            capture_output=True, text=True, timeout=60)
    text = result.stdout + result.stderr
    (out/(label+'.txt')).write_text('\n'.join(line.rstrip() for line in text.splitlines()).rstrip()+'\n')
    results.append(dict(case=label, exit_code=result.returncode, expected=expected, passed=result.returncode==expected))
    assert result.returncode == expected, (label, result.returncode, text[-3000:])
    return result

def hashes(root, names):
    return {name: hashlib.sha256((root/name).read_bytes()).hexdigest() for name in names}

invoke('usage', [], 2)
with tempfile.TemporaryDirectory(prefix='probe_cli_', dir='target') as directory:
    root = Path(directory).resolve()
    empty = root/'empty'
    empty.mkdir()
    invoke('empty-input', ['--free', str(empty)], 1)
    assert not (empty/'pubspec.yaml').exists()
    package = root/'package'
    package.mkdir()
    (package/'main.dart').write_text('void main() { int unused = 1; }\n')
    (root/'outside.dart').write_text('void main() {}\n')
    invoke('missing-input', ['--free', str(package), 'missing.dart'], 1)
    invoke('duplicate-input', ['--free', str(package), 'main.dart', './main.dart'], 1)
    invoke('outside-input', ['--free', str(package), '../outside.dart'], 1)
    assert not (package/'pubspec.yaml').exists()
    warning = invoke('fresh-warning-match', ['--free', str(package)], 0)
    assert 'total: = 1 | ~ 0 | - 0 | + 0' in warning.stdout
    metadata = ['pubspec.yaml', '.dart_tool/package_config.json']
    before = hashes(package, metadata)
    invoke('repeated-free-match', ['--free', str(package)], 0)
    assert hashes(package, metadata) == before
    invoke('invalid-ours-sdk', ['--free', str(package)], 1,
           dict(DARTFORGE_SDK_LIB=str(root/'missing-sdk')))
    assert hashes(package, metadata) == before
    (package/'main.dart').write_text('void main() {}\n')
    (package/'pubspec.yaml').write_text('name: user_project\npublish_to: none\nenvironment:\n  sdk: ^3.6.0\n# preserve this comment\n')
    (package/'.dart_tool/package_config.json').write_text(json.dumps(dict(configVersion=2, generator='user', packages=[dict(name='user_project',rootUri='../',languageVersion='3.6')])))
    before = hashes(package, metadata)
    invoke('existing-metadata-match', ['--free', str(package)], 0)
    assert hashes(package, metadata) == before

with tempfile.TemporaryDirectory(prefix='probe_gate_', dir='corpus/diagnostics') as directory:
    root = Path(directory).resolve()
    name = root.name
    source = b'void main() {}\n'
    (root/'main.dart').write_bytes(source)
    (root/'pubspec.yaml').write_text(f'name: corpus_{name}\nenvironment:\n  sdk: ^3.6.0\n')
    group = dict(sdk='3.6.2', packages=[], origin='deliberate negative gate fixture')
    (root/'group.json').write_text(json.dumps(group))
    value = 0xcbf29ce484222325
    for byte in b'main.dart\0'+source:
        value = ((value^byte)*0x100000001b3)&((1<<64)-1)
    meta = dict(sdk='3.6.2', hash=f'{value:016x}', files=1, diagnostics=1)
    record = dict(file='main.dart',offset=0,length=4,code='intentional_oracle_only',severity='ERROR',type='COMPILE_TIME_ERROR',line=1,column=1,problemMessage='Deliberate negative gate fixture.')
    (root/'oracle.jsonl').write_text(json.dumps(record)+'\n')
    (root/'oracle.json').write_text(json.dumps(meta))
    mismatch = invoke('recorded-mismatch', [name,'main.dart'], 1)
    assert 'total: = 0 | ~ 0 | - 1 | + 0' in mismatch.stdout
    (root/'main.dart').write_text('void main() { print(1); }\n')
    invoke('stale-source', [name,'main.dart'], 1)
    (root/'main.dart').write_bytes(source)
    meta['diagnostics'] = 0
    (root/'oracle.json').write_text(json.dumps(meta))
    invoke('corrupt-reference-count', [name,'main.dart'], 1)
    meta['diagnostics'] = 2
    (root/'oracle.json').write_text(json.dumps(meta))
    (root/'oracle.jsonl').write_text((json.dumps(record)+'\n')*2)
    duplicate = invoke('duplicate-reference', [name,'main.dart'], 1)
    assert 'total: = 0 | ~ 0 | - 2 | + 0' in duplicate.stdout
    preserved = ['main.dart','pubspec.yaml','group.json','oracle.json','oracle.jsonl']
    group['sdk'] = '3.13.4'
    (root/'group.json').write_text(json.dumps(group))
    before = hashes(root, preserved)
    wrong = invoke('wrong-recorder-sdk', ['oracle','--group',name], 1,
                   dict(DARTFORGE_ORACLE_3_13=str(sdk)), main=True)
    assert 'expected Dart SDK 3.13.4, got 3.6.2' in wrong.stderr
    assert hashes(root, preserved) == before
    group['sdk'] = '3.6.2'
    (root/'group.json').write_text(json.dumps(group))
    with tempfile.TemporaryDirectory(prefix='mock_oracle_', dir='target') as mock_directory:
        mock = Path(mock_directory).resolve()
        (mock/'bin').mkdir()
        rust = mock/'invalid_report.rs'
        rust.write_text('fn main() { if std::env::args().nth(1).as_deref()==Some("--version") { println!("Dart SDK version: 3.6.2 (test double)"); } else { println!("{}", r#"{"version":99,"diagnostics":[]}"#); } }\n')
        subprocess.run(['rustc','--edition=2024',str(rust),'-o',str(mock/'bin'/('dart'+suffix))], check=True)
        before = hashes(root, preserved)
        failed = invoke('invalid-oracle-output-preserves-data', ['oracle','--group',name], 1,
                        dict(DARTFORGE_ORACLE_3_6=str(mock)), main=True)
        assert 'preserved' in failed.stderr
        assert hashes(root, preserved) == before
invoke('unknown-recorder-group', ['oracle','--group','unknown_probe_gate_group'], 1, main=True)
(out/'cli-checks.json').write_text(json.dumps(dict(cases=results, all_passed=all(r['passed'] for r in results),
    real_sdk=str(sdk), warning_match_verified=True, existing_metadata_preserved=True,
    sdk_failure_preserved_sources_and_oracles=True, invalid_report_test_uses_a_test_double=True), indent=2)+'\n')
print(len(results), 'CLI checks passed, including real SDK diagnostics and failure preservation.')

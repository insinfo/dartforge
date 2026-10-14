"""Isolate getter devirtualization in File Explorer; compile only, no timers or GUI."""
from pathlib import Path
import ast
import gzip
import hashlib
import json
import os
import re
import struct
import subprocess

root = Path.cwd()
out = root / 'bench/results/2026-10-14-physical-getter-production/isolated-file-explorer'
work = root / 'target/isolated-getter-file-explorer'
work.mkdir(parents=True, exist_ok=True)
previous_path = root / 'target/canonical-cache-sizes/comparison.json'
previous = json.loads(previous_path.read_text())
on = previous['profiles']['combined_default']
compiler = Path(previous['compiler'])
sha = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
inputs_path = root / 'target/canonical-cache-sizes/inputs.json'
inputs = json.loads(inputs_path.read_text())
assert sha(compiler) == previous['compiler_sha256'] == 'bee770c7623e94439a5df1d5b792af9c53577c682398c6491c379befe51ef171'
assert sha(inputs_path) == previous['input_manifest_sha256']
assert len(inputs) == previous['input_count']
assert all(sha(path) == digest for path, digest in inputs.items())
assert on['exit'] == 0 and sha(on['binary']) == on['binary_sha256']
assert all(on[key] is None for key in ['NO_CANONICAL_CACHE_PROVENANCE', 'NO_PHYSICAL_GETTER_DEVIRTUALIZATION', 'NO_ENUM_PATTERN_IDENTITY'])
archives = on['linked_static_archives_sha256']
assert all(sha(path) == digest for path, digest in archives.items())
producer = json.loads((compiler.parent / 'provenance.json').read_text())
source_inputs = producer.get('source_hashes', {})
assert all(sha(root / path) == digest for path, digest in source_inputs.items())
# sizes.py was launched from the same clean environment; refuse hidden codegen overrides.
assert not any(key.startswith('DARTFORGE_') for key in os.environ)
assert 'CARGO_TARGET_DIR' not in os.environ
env = dict(os.environ, **previous['environment'])
for key in previous['unset_environment']:
    env.pop(key, None)
env.pop('DARTFORGE_NO_ENUM_PATTERN_IDENTITY', None)
env.pop('DARTFORGE_NO_CANONICAL_CACHE_PROVENANCE', None)
env['DARTFORGE_NO_PHYSICAL_GETTER_DEVIRTUALIZATION'] = '1'
env['DARTFORGE_WHY'] = str(work / 'retention.tsv')
command = list(on['command'])
assert command[-2] == '-o'
binary = work / 'program'
command[-1] = str(binary)
ledger = {'compiler': str(compiler), 'compiler_sha256': sha(compiler),
          'reused_on_ledger': str(previous_path), 'reused_on_ledger_sha256': sha(previous_path),
          'input_manifest': str(inputs_path), 'input_manifest_sha256': sha(inputs_path),
          'input_count': len(inputs), 'linked_static_archives_sha256': archives,
          'source_hashes': source_inputs, 'cwd': str(root), 'command': command,
          'environment': {key: value for key, value in env.items() if key.startswith('DARTFORGE_') or key in ['TMPDIR', 'LLVM_SYS_221_PREFIX']},
          'unset_environment': previous['unset_environment'] + ['DARTFORGE_NO_ENUM_PATTERN_IDENTITY', 'DARTFORGE_NO_CANONICAL_CACHE_PROVENANCE'],
          'reuse_certificate': 'Same immutable compiler, complete input inventory, runtime archive and canonical/enum defaults. Clean inherited environment confirmed by original builder and asserted here. Only getter opt-out changes code; output and retention paths differ for evidence.',
          'on': on, 'complete': False,
          'scope': 'Compile-only matched file bytes, Mach-O sections and IR call counts. No timers, execution, GUI or throughput claim.'}
path = out / 'comparison.json'
path.write_text(json.dumps(ledger, indent=2) + '\n')
print('isolated getter OFF build starting', flush=True)
result = subprocess.run(command, cwd=root, env=env, capture_output=True, timeout=1800)
(out / 'off-build.stdout').write_bytes(result.stdout)
(out / 'off-build.stderr').write_bytes(result.stderr)
ledger['off'] = {'exit': result.returncode, 'stdout_sha256': hashlib.sha256(result.stdout).hexdigest(),
                 'stderr_sha256': hashlib.sha256(result.stderr).hexdigest()}
if result.returncode == 0:
    # Reuse only the original builder's pure Mach-O section reader, not its build loop.
    source = (root / 'bench/results/2026-10-14-native-canonical-cache/sizes.py').read_text()
    definition = next(node for node in ast.parse(source).body if isinstance(node, ast.FunctionDef) and node.name == 'sections')
    namespace = {'struct': struct}
    exec(ast.get_source_segment(source, definition), namespace)
    sections = namespace['sections'](binary)
    ledger['off'].update(binary=str(binary), binary_bytes=binary.stat().st_size, binary_sha256=sha(binary), sections=sections)
    ledger['deltas_on_minus_off'] = {'binary_bytes': on['binary_bytes'] - binary.stat().st_size,
                                    'text_section_bytes': on['text_section_bytes'] - sections['__TEXT,__text']['bytes']}
    irs = {}
    for mode, folder in [('on', Path(on['binary']).parent), ('off', work)]:
        ir_path = folder / '.df_tmp/program.ll'
        if ir_path.exists():
            ir = ir_path.read_bytes()
            irs[mode] = {'sha256': sha(ir_path), 'bytes': len(ir),
                         'selector_lookup_calls': len(re.findall(rb'call ptr @df\.selector(?:_d)?\(', ir))}
    ledger['ir'] = irs
    linkmap = binary.with_suffix('.map')
    if linkmap.exists():
        text = linkmap.read_text()
        assert all(str(archive) in text for archive in archives)
        ledger['off'].update(link_map=str(linkmap), link_map_sha256=sha(linkmap))
ledger['compiler_unchanged'] = sha(compiler) == previous['compiler_sha256']
ledger['inputs_unchanged'] = all(sha(path) == digest for path, digest in inputs.items())
ledger['runtime_archives_unchanged'] = all(sha(path) == digest for path, digest in archives.items())
ledger['sources_unchanged'] = all(sha(root / path) == digest for path, digest in source_inputs.items())
ledger['complete'] = True
path.write_text(json.dumps(ledger, indent=2) + '\n')
print('isolated getter OFF terminal', result.returncode, ledger.get('deltas_on_minus_off'), flush=True)
assert result.returncode == 0 and all(ledger[key] for key in ['compiler_unchanged', 'inputs_unchanged', 'runtime_archives_unchanged', 'sources_unchanged'])

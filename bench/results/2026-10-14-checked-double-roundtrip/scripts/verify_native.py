"""Run actual same-producer tracing ON/OFF correctness; no timings are measured."""
from pathlib import Path
import argparse, hashlib, json, os, re, shutil, subprocess, traceback, collections
parser = argparse.ArgumentParser()
parser.add_argument('--producer', required=True)
parser.add_argument('--producer-sha256', required=True)
parser.add_argument('--output', required=True)
args = parser.parse_args()
root = Path.cwd()
producer = Path(args.producer).resolve()
out = Path(args.output).resolve()
out.mkdir(parents=True, exist_ok=False)
sdk = Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk')
package = Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui/examples/file_explorer/.dart_tool/package_config.json')
actual_format = Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui/lib/src/graphics/image/codecs/color/format.dart')
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(producer) == args.producer_sha256
source_files = [root/'crates/emit_native/tests/fixtures/checked_double_roundtrip/main.dart', root/'target/enum-pattern-identity/format_probe.dart']
metadata_files = [p for p in [producer.parent/'source.json', producer.parent/'source.patch'] if p.is_file()]
inputs = {str(p): sha(p) for p in [producer, package, actual_format, *source_files, *metadata_files]}

ledger = {'validator_sha256': sha(Path(__file__).resolve()), 'producer_source_metadata': {str(p):sha(p) for p in metadata_files}, 'sdk_executable_sha256':sha(sdk/'bin/dart'), 'producer_sha256': sha(producer), 'inputs_before': inputs, 'commands': [], 'cases': [], 'scope': 'Canonical checked-double return reuse, tracing fixed AOT only. Runtime execution/oracle and static IR counts; no timing or allocation claim.'}
base = dict(os.environ, TMPDIR=str(out), DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang')
for key in ['DARTFORGE_MEMORY', 'DARTFORGE_GC_STRESS', 'DARTFORGE_ARC_PREPARED', 'DARTFORGE_ARC_PREPARE', 'DARTFORGE_ARC_CHECK', 'DARTFORGE_EFFECTS', 'DARTFORGE_NO_CHECKED_DOUBLE_ROUNDTRIP', 'DARTFORGE_NO_ENUM_PATTERN_IDENTITY', 'DARTFORGE_KEEP_IR', 'DARTFORGE_LINK_MAP']:
    base.pop(key, None)
def save():
    (out/'validation.json').write_text(json.dumps(ledger, indent=2)+'\n')
def run(label, command, env, timeout=1200):
    with (out/(label+'.stdout')).open('wb') as stdout, (out/(label+'.stderr')).open('wb') as stderr:
        result = subprocess.run(command, env=env, stdout=stdout, stderr=stderr, timeout=timeout)
    record = {'label': label, 'command': command, 'exit_code': result.returncode, 'stdout_sha256': sha(out/(label+'.stdout')), 'stderr_sha256': sha(out/(label+'.stderr')), 'environment': {k: env.get(k) for k in ['TMPDIR','DARTFORGE_NO_CHECKED_DOUBLE_ROUNDTRIP','DARTFORGE_KEEP_IR','DARTFORGE_LINK_MAP','DARTFORGE_GC_STRESS']}}
    ledger['commands'].append(record)
    save()
    print(label, result.returncode, flush=True)
    return record
try:
    for name, source, function in [('edge',source_files[0],'roundTrip'),('format',source_files[1],'convertFormatValue')]:
        directory = out/name
        directory.mkdir()
        entry = directory/'main.dart'
        shutil.copy2(source, entry)
        packages = ['--packages='+str(package)] if name == 'format' else []
        r = run(name+'-vm', [str(sdk/'bin/dart'), *packages, str(entry)], base, 60)
        assert r['exit_code'] == 0
        expected = (out/(name+'-vm.stdout')).read_bytes()
        assert (out/(name+'-vm.stderr')).stat().st_size == 0
        official = directory/'official'
        r = run(name+'-official-compile', [str(sdk/'bin/dart'),'compile','exe',*packages,str(entry),'-o',str(official)],base)
        assert r['exit_code'] == 0
        r = run(name+'-official',[str(official)],base,60)
        assert r['exit_code'] == 0 and (out/(name+'-official.stdout')).read_bytes() == expected
        assert (out/(name+'-official.stderr')).stat().st_size == 0
        ledger.setdefault('official', {})[name] = {'artifact_sha256':sha(official),'oracle_stdout_sha256':hashlib.sha256(expected).hexdigest()}
        for enabled in [False,True]:
            label = name+('-on' if enabled else '-off')
            case = directory/('on' if enabled else 'off')
            case.mkdir()
            env = dict(base, DARTFORGE_KEEP_IR='1',DARTFORGE_LINK_MAP='1')
            if not enabled: env['DARTFORGE_NO_CHECKED_DOUBLE_ROUNDTRIP'] = '1'
            command = [str(producer),'compile-native',str(entry),'--sdk',str(sdk/'lib'),'--optimize','--memory=tracing','--exceptions=check','--roots=shadow','--stack-trace=none','-o',str(case/'program')]
            if name == 'format': command.extend(['--packages',str(package)])
            r = run(label+'-compile',command,env)
            row = {'fixture':name,'enabled':enabled,'compile_exit':r['exit_code']}
            ledger['cases'].append(row)
            assert r['exit_code'] == 0
            row['artifact_sha256'] = sha(case/'program')
            row['contract_manifest_sha256'] = sha(case/'program.contracts.json')
            row['executions'] = []
            for stress in [False, True]:
                e = env.copy()
                if stress: e['DARTFORGE_GC_STRESS'] = '1'
                mode = 'stress' if stress else 'normal'
                execution = run(label+'-'+mode,[str(case/'program')],e,60)
                exact = (out/(label+'-'+mode+'.stdout')).read_bytes() == expected
                execution.update(exact_VM=exact, stderr_bytes=(out/(label+'-'+mode+'.stderr')).stat().st_size)
                row['executions'].append(execution)
                save()
                assert execution['exit_code'] == 0 and exact and execution['stderr_bytes'] == 0
            ir = case/'.df_tmp/program.ll'
            text = ir.read_text()
            matches = re.findall(r'define[^\n]*@[^\n]*\.'+function+r'\([^\n]*\).*?\n\}',text,re.S)
            assert len(matches)==1,(label,len(matches))
            body = matches[0]
            (case/(function+'.ll')).write_text(body+'\n')
            counts = collections.Counter(re.findall(r'\bcall\b[^\n]*?@([^\s(]+)\(',body))
            row['IR'] = {'full_sha256':sha(ir),'function_sha256':sha(case/(function+'.ll')),'static_call_counts':dict(counts),'root_slot_stores':len(re.findall(r'\bstore i64[^\n]*ptr %gcs',body)), 'scope':'Static pre-LLVM callsite/rootstore counts, not machine execution counts or consumed allocations.'}
            row['map_sha256'] = sha(case/'program.map')
            save()
    for name, expected_removed in [('edge',1),('format',8)]:
        pair = {row['enabled']:row['IR']['static_call_counts'] for row in ledger['cases'] if row['fixture']==name}
        assert pair[False].get('df.box_double',0)-pair[True].get('df.box_double',0)==expected_removed, (name,pair)
        assert pair[False].get('df.unbox_double',0)==pair[True].get('df.unbox_double',0), (name,pair)
        assert pair[False].get('df.selector',0)==pair[True].get('df.selector',0), (name,pair)
    ledger.update(result='PASS',inputs_unchanged=all(sha(Path(p))==h for p,h in inputs.items()),native_executions=8)
    assert ledger['inputs_unchanged']
except BaseException as error:
    ledger['result']='FAIL'
    ledger['failure']=repr(error)
    ledger['traceback']=traceback.format_exc()
    save()
    raise
save()
print('PASS 8 native runs',flush=True)

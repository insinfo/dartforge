#!/usr/bin/env python3
"""Independently check real published artifacts, final bodies/ABI and closure graph."""
import collections,hashlib,json,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4];OUT=Path(__file__).resolve().parent
CACHE=ROOT/'target/allocation-closure-publication-cache/sdk'
TOOL=ROOT/'target/allocation-closure-publication-artifacts/hash-tool'
directories=[p for p in CACHE.iterdir() if (p/'pronto').exists()]
assert len(directories)==1,directories
BUNDLE=directories[0];FILE=BUNDLE/'allocation-inventory.json';inventory=json.loads(FILE.read_text())
ready=json.loads((BUNDLE/'allocation-inventory.ready').read_text())
assert inventory['schema']==1 and inventory['production'] is True and inventory['cache_key']==BUNDLE.name
runtime=[]
for output in (ROOT/'target/jit-readiness-build/debug/build').glob('dartforge-emit-native-*/output'):
    values={}
    for line in output.read_text().splitlines():
        if line.startswith('cargo::rustc-env='):
            key,value=line.removeprefix('cargo::rustc-env=').split('=',1);values[key]=value
    if values.get('DARTFORGE_EMITTER_HASH')==inventory['compiler']:
        runtime.append(Path(values['DARTFORGE_RUNTIME_BUILD_DIR'])/values['DARTFORGE_RUNTIME_PRODUCTION'])
runtime=list(dict.fromkeys(runtime));assert runtime,'exact compiler build runtime not found'
paths=[FILE,ROOT/'crates/runtime/effects.tsv']+runtime+[BUNDLE/a['name'] for a in inventory['artifacts']]
paths += [BUNDLE/(m['module'].split(':',1)[1]+'.ll') for m in inventory['inventory']['modules']]
output=subprocess.check_output([str(TOOL),*map(str,paths)],text=True)
digests={Path(line.split('\t',1)[1]):line.split('\t',1)[0] for line in output.splitlines()}
assert ready=={'schema':1,'cache_key':BUNDLE.name,'inventory_digest':digests[FILE]}
assert inventory['runtime']['effect_catalog_digest']==digests[ROOT/'crates/runtime/effects.tsv']
matching_runtime=[p for p in runtime if digests[p]==inventory['runtime']['artifact_digest']];assert matching_runtime
for artifact in inventory['artifacts']:assert digests[BUNDLE/artifact['name']]==artifact['digest'],artifact
functions=inventory['inventory']['functions'];globals_=inventory['inventory']['globals']
function_records={(f['module'],f['symbol']):f for f in functions}
global_records={(g['module'],g['symbol']):g for g in globals_}
assert len(function_records)==len(functions) and len(global_records)==len(globals_)
frames=[];body_hash_file=ROOT/'target/allocation-closure-publication-artifacts/actual-body-digests.txt'
with body_hash_file.open('w') as hashes:
    process=subprocess.Popen([str(TOOL),'--frames'],stdin=subprocess.PIPE,stdout=hashes)
    def frame(value,kind,module,symbol,expected):
        data=value.encode();process.stdin.write(f'{len(data)}\n'.encode());process.stdin.write(data)
        frames.append((kind,module,symbol,expected))
    for module in inventory['inventory']['modules']:
        uri=module['module'];path=BUNDLE/(uri.split(':',1)[1]+'.ll')
        assert digests[path]==module['final_ir_digest']
        lines=iter(path.read_text().splitlines())
        for header in lines:
            if header.startswith('@') and ' = ' in header:
                symbol=header.split(' = ',1)[0][1:].strip('"');record=global_records[(uri,symbol)]
                frame(header,'global',uri,symbol,record['initializer_digest'])
            if not header.startswith('define '):continue
            prefix,tail=header.split('@',1);symbol=tail.split('(',1)[0].strip('"');record=function_records[(uri,symbol)]
            text=header+'\n';structural=header.split(';',1)[0]
            if not ('{' in structural and '}' in structural.split('{',1)[1]):
                for line in lines:
                    text+=line+'\n'
                    if line.strip()=='}':break
            frame(text,'function',uri,symbol,record['body_digest'])
            if record['abi'] is not None:
                actual_result=prefix.split()[-1]
                actual_args=tail.split('(',1)[1].split(')',1)[0]
                actual_parameters=[] if not actual_args.strip() else [arg.strip().split()[0] for arg in actual_args.split(',')]
                assert record['abi']=={'result':actual_result,'parameters':actual_parameters},(uri,symbol)
    process.stdin.close();assert process.wait()==0
actual=body_hash_file.read_text().splitlines();assert len(actual)==len(frames)
for digest,(kind,module,symbol,expected) in zip(actual,frames):assert digest==expected,(kind,module,symbol)
assert sum(kind=='function' for kind,_,_,_ in frames)==len(functions)
assert sum(kind=='global' for kind,_,_,_ in frames)==len(globals_)
# Independent reverse-graph poison propagation and finite-set union. This does
# not rerun the Rust SCC implementation or infer facts from return types.
keys=collections.defaultdict(list)
for index,function in enumerate(functions):keys[function['key']].append(index)
edges=[];reverse=[[] for _ in functions];bad=set()
for index,function in enumerate(functions):
    if function['unknown_obligations'] or len(keys[function['key']])!=1:bad.add(index)
    targets=[]
    for name in function['dependencies']:
        if len(keys[name])!=1:bad.add(index);continue
        target=keys[name][0];targets.append(target);reverse[target].append(index)
    edges.append(targets)
queue=collections.deque(bad)
while queue:
    for caller in reverse[queue.popleft()]:
        if caller not in bad:bad.add(caller);queue.append(caller)
classes=[set(f['direct_classes']) for f in functions]
queue=collections.deque(index for index,cids in enumerate(classes) if cids and index not in bad)
while queue:
    target=queue.popleft()
    for caller in reverse[target]:
        if caller in bad:continue
        before=len(classes[caller]);classes[caller].update(classes[target])
        if len(classes[caller])!=before:queue.append(caller)
for index,function in enumerate(functions):
    expected=None if index in bad else sorted(classes[index])
    assert function['transitive_classes']==expected,function['key']
legacy=collections.Counter()
for path in BUNDLE.glob('*.poda'):
    assert path.read_text().splitlines()[0].startswith('dartforge-poda\t8\t')
    for line in path.read_text().splitlines():
        if line.startswith('I\t'):legacy[line.split('\t')[2]]+=1
reasons=collections.Counter(reason for f in functions for reason in f['unknown_obligations'])
exposures=collections.Counter(reason for f in functions for reason in f['exposure_obligations'])
artifacts=[{'name':a['name'],'size':(BUNDLE/a['name']).stat().st_size,'blake3':digests[BUNDLE/a['name']],
            'sha256':hashlib.sha256((BUNDLE/a['name']).read_bytes()).hexdigest()} for a in inventory['artifacts']]
report={'cache':str(BUNDLE.relative_to(ROOT)),'schema':1,'profile':'production','compiler':inventory['compiler'],
        'sidecar_size':FILE.stat().st_size,'sidecar_blake3':digests[FILE],
        'sidecar_sha256':hashlib.sha256(FILE.read_bytes()).hexdigest(),
        'runtime':{'matching_artifacts':[str(p.relative_to(ROOT)) for p in matching_runtime],**inventory['runtime']},
        'checks':{'ready_digest':True,'all_30_object_summary_hashes':len(artifacts)==30,'all_final_module_hashes':True,
                  'all_actual_function_bodies_and_supported_abi':True,'all_actual_global_initializers':True,'independent_transitive_graph_closure':True},
        'counts':{'modules':len(inventory['inventory']['modules']),'functions':len(functions),'globals':len(globals_),
                  'transitive_known_empty':sum(f['transitive_classes']==[] for f in functions),
                  'transitive_known_nonempty':sum(bool(f['transitive_classes']) for f in functions),
                  'transitive_unknown':sum(f['transitive_classes'] is None for f in functions),
                  'direct_nonempty_functions':sum(bool(f['direct_classes']) for f in functions),'legacy_poda_states':dict(legacy)},
        'unknown_obligations':reasons.most_common(30),'exposure_obligations':exposures.most_common(),
        'artifacts':artifacts,'modules':inventory['inventory']['modules'],
        'scope':'Actual fresh SDK compile and publication verified; no import class pruning or native size/performance benefit claimed.'}
report['body_verification_stream']={'path':str(body_hash_file.relative_to(ROOT)),
    'sha256':hashlib.sha256(body_hash_file.read_bytes()).hexdigest(),'records':len(frames)}
for file in matching_runtime:
    report['runtime'].setdefault('sha256',{})[str(file.relative_to(ROOT))]=hashlib.sha256(file.read_bytes()).hexdigest()
sdk=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib')
sdk_files={str(file.relative_to(sdk)):hashlib.sha256(file.read_bytes()).hexdigest()
    for file in sorted(sdk.rglob('*')) if file.is_file() and file.suffix in ('.dart','.json')}
report['official_sdk_source']={'path':str(sdk),'version':(sdk.parent/'version').read_text().strip(),
    'files':len(sdk_files),'tree_sha256':hashlib.sha256(json.dumps(sdk_files,sort_keys=True,separators=(',',':')).encode()).hexdigest(),
    'libraries_json_sha256':sdk_files['libraries.json'],
    'captured':'after the gate; the cache key and final IR bind the compilation inputs'}
(OUT/'published-bundle.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'checks':report['checks'],'counts':report['counts'],'unknown_top':report['unknown_obligations'][:8]},indent=2))

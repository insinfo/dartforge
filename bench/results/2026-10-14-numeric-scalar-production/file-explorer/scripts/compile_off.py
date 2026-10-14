"""Compile the unchanged ordinary explorer; compilation is not GUI validation."""
from pathlib import Path
import hashlib, json, os, subprocess, time
root=Path.cwd();out=root/'target/numeric-scalar-production/file-explorer-off';out.mkdir(parents=True,exist_ok=False)
producer=root/'target/numeric-scalar-producer/dartforge';sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(producer)=='1d2dadbc030f9da15f4672900ad1f9aab1ea736edb0acff6be82174b8d620655'
prior=json.loads((root/'target/canonical-cache-sizes/comparison.json').read_text());manifest=json.loads((root/'target/canonical-cache-sizes/inputs.json').read_text());inputs={p:sha(Path(p)) for p in manifest};(out/'inputs.json').write_text(json.dumps(inputs,indent=2)+'\n')
entry=Path(prior['entry']);packages=Path(prior['package_config']);env=os.environ.copy()
for key in prior['unset_environment']+['DARTFORGE_NO_CHECKED_DOUBLE_ROUNDTRIP','DARTFORGE_NO_SCALAR_COALESCE','DARTFORGE_NO_PHYSICAL_GETTER_DEVIRTUALIZATION','DARTFORGE_NO_ENUM_PATTERN_IDENTITY','DARTFORGE_NO_CANONICAL_CACHE_PROVENANCE']:
    env.pop(key,None)
env.update(prior['environment'])
for key in ['DARTFORGE_NO_PHYSICAL_GETTER_DEVIRTUALIZATION','DARTFORGE_NO_ENUM_PATTERN_IDENTITY']:
    env.pop(key,None)
env['TMPDIR']=str(out)
env['DARTFORGE_NO_SCALAR_COALESCE']='1'
env['DARTFORGE_NO_CHECKED_DOUBLE_ROUNDTRIP']='1'
command=[str(producer),'compile-native',str(entry),'--sdk',env['DARTFORGE_SDK_LIB'],'--packages',str(packages),'--optimize','--memory=tracing','--exceptions=check','--roots=shadow','--stack-trace=none','-o',str(out/'program')]
ledger={'command':command,'producer_sha256':sha(producer),'entry_sha256':sha(entry),'package_config_sha256':sha(packages),'input_count':len(inputs),'input_manifest_sha256':sha(out/'inputs.json'),'differences_from_historical_input_manifest':[p for p,h in inputs.items() if manifest[p]!=h],'environment':{k:env.get(k) for k in sorted(set(prior['environment'])|set(prior['unset_environment'])|{'DARTFORGE_NO_CHECKED_DOUBLE_ROUNDTRIP','DARTFORGE_NO_SCALAR_COALESCE','DARTFORGE_NO_ENUM_PATTERN_IDENTITY','DARTFORGE_NO_PHYSICAL_GETTER_DEVIRTUALIZATION'})},'scope':'New paired numeric-OFF ordinary original-entry FileExplorer compile; both scalar coalesce and checked-double roundtrip disabled, other optimizations defaultON. No throughput/startup/fullfeatureclaim; GUI pending. Historical artifact has different producer and is not samecompiler ablation.'}
def save():(out/'validation.json').write_text(json.dumps(ledger,indent=2)+'\n')
save()
with (out/'compile.stdout').open('wb') as stdout,(out/'compile.stderr').open('wb') as stderr:
    result=subprocess.run(command,env=env,stdout=stdout,stderr=stderr,timeout=1800)
ledger.update(exit_code=result.returncode,inputs_unchanged=all(sha(Path(p))==h for p,h in inputs.items()),producer_unchanged=sha(producer)==ledger['producer_sha256'])
if result.returncode==0:
    for filename in ['program','program.map','program.contracts.json']:
        p=out/filename;ledger[filename]={'path':str(p),'bytes':p.stat().st_size,'sha256':sha(p)}
    r=subprocess.run(['otool','-l',str(out/'program')],stdout=(out/'otool.stdout').open('wb'),stderr=(out/'otool.stderr').open('wb'));ledger['otool_exit']=r.returncode
save();print(json.dumps({k:ledger.get(k) for k in ['exit_code','input_count','inputs_unchanged','producer_unchanged','program']},indent=2),flush=True)
assert result.returncode==0 and ledger['inputs_unchanged'] and ledger['producer_unchanged']

import hashlib, json, os, subprocess
from pathlib import Path
root=Path.cwd()
out=root/'bench/results/2026-10-14-native-vector-pooling/fixtures'
out.mkdir(parents=True,exist_ok=True)
compiler=root/'target/native-provenance/pooled-indexed-compiler/dartforge'
sdk=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib')
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
base=os.environ.copy()
for key in ['DARTFORGE_MEMORY','DARTFORGE_GC_STRESS','DARTFORGE_ARC_PREPARED','DARTFORGE_AOT_CONST_ARRAY_POOLING']:base.pop(key,None)
base.update({'DARTFORGE_ARC_CHECK':'1','DARTFORGE_EFFECTS':'check'})
ledger={'complete':False,'compiler_sha256':sha(compiler),'runs':[],'timing_measured':False,'sources':{}}
def save():(out/'validation.json').write_text(json.dumps(ledger,indent=2)+'\n')
def run(label,cmd,env):
 result=subprocess.run([str(v) for v in cmd],env=env,capture_output=True,timeout=1200)
 (out/(label+'.stdout')).write_bytes(result.stdout);(out/(label+'.stderr')).write_bytes(result.stderr)
 row={'label':label,'command':[str(v) for v in cmd],'environment':{k:v for k,v in env.items() if k.startswith('DARTFORGE_')},'exit_code':result.returncode,'stdout_sha256':hashlib.sha256(result.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(result.stderr).hexdigest()};ledger['runs'].append(row);save();assert result.returncode==0,label
 return result,row
for name in ['pending_field_phi','null_phi_coalesce','cross_image_dispatch']:
 source=root/('crates/emit_native/tests/fixtures/arc_ports/'+name+'.dart');ledger['sources'][str(source)]=sha(source)
 oracle,_=run(name+'-vm',[sdk.parent/'bin/dart',source],base);assert not oracle.stderr
 for mode in ['tracing','arc','prepared']:
  for pool in ([True,False] if mode=='prepared' else [True]):
   label=name+'-'+mode+('-on' if pool else '-off');env=base.copy()
   env['DARTFORGE_AOT_CONST_ARRAY_POOLING']='1' if pool else '0'
   if mode=='prepared':env['DARTFORGE_ARC_PREPARED']='1'
   binary=root/('target/native-provenance/pooled-indexed-fixtures/'+label+'/program');binary.parent.mkdir(parents=True,exist_ok=True)
   _,row=run(label+'-build',[compiler,'compile-native',source,'--sdk',sdk,'--optimize','--memory='+('tracing' if mode=='tracing' else 'arc'),'--exceptions=check','--roots=shadow','--stack-trace=none','-o',binary],env)
   row.update({'binary_bytes':binary.stat().st_size,'binary_sha256':sha(binary)});save()
   for stress in [False,True]:
    execution=env.copy()
    if stress:execution['DARTFORGE_GC_STRESS']='1'
    result,row=run(label+('-stress' if stress else '-normal'),[binary],execution)
    row['exact_vm']=result.stdout==oracle.stdout and not result.stderr;save();assert row['exact_vm'],label
   print(label+': VM exact normal+stress',flush=True)
ledger['inputs_unchanged']=sha(compiler)==ledger['compiler_sha256'] and all(sha(p)==h for p,h in ledger['sources'].items());assert ledger['inputs_unchanged'];ledger['complete']=True;save()

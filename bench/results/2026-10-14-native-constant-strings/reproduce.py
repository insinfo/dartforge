import pathlib,subprocess,hashlib,json,os,sys
root=pathlib.Path.cwd();out=root/'bench/results/2026-10-14-native-constant-strings';base=root/'target/native-provenance/constant-strings';compiler=pathlib.Path(sys.argv[1]);baseline=root/'target/native-provenance/latin1-type-identity-compiler/dartforge';sdk=pathlib.Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib')
sha=lambda p:hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
ledger={'complete':False,'scope':'Ordinary string and actual UTF8/JSON correctness; linked symbol sizes, no performance or broad compatibility claim.','compiler_sha256':sha(compiler),'baseline_compiler_sha256':sha(baseline),'runs':[],'sources':{}}
env=os.environ.copy();env.update({'DARTFORGE_CLANG':'/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang','LLVM_SYS_221_PREFIX':'/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64','TMPDIR':str(base)})
for k in ['DARTFORGE_GC_STRESS','DARTFORGE_ARC_PREPARED','DARTFORGE_MEMORY']:env.pop(k,None)
env['DARTFORGE_ARC_CHECK']='1';env['DARTFORGE_EFFECTS']='check';env['DARTFORGE_LINK_MAP']='1'
def save():(out/'validation.json').write_text(json.dumps(ledger,indent=2)+'\n')
def run(tag,args,e):
 r=subprocess.run(list(map(str,args)),capture_output=True,env=e,timeout=1200);(out/(tag+'.stdout')).write_bytes(r.stdout);(out/(tag+'.stderr')).write_bytes(r.stderr);row={'label':tag,'command':list(map(str,args)),'exit':r.returncode,'stdout_sha256':hashlib.sha256(r.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(r.stderr).hexdigest()};ledger['runs'].append(row);save();return r,row
for label,source in [('ordinary',root/'crates/emit_native/tests/fixtures/constant_string_interpolation.dart'),('utf8',out/'utf8_probe.dart')]:
 ledger['sources'][label]={'path':str(source),'sha256':sha(source)};oracle,_=run(label+'.VM',[sdk.parent/'bin/dart',source],env);assert oracle.returncode==0 and not oracle.stderr
 for mode in ['baseline-tracing','tracing','arc','prepared']:
  e=env.copy()
  if mode=='prepared':e['DARTFORGE_ARC_PREPARED']='1'
  folder=base/label/mode;folder.mkdir(parents=True,exist_ok=True);binary=folder/'program';tool=baseline if mode=='baseline-tracing' else compiler
  r,row=run(label+'.'+mode+'.build',[tool,'compile-native',source,'--sdk',sdk,'--optimize','--exceptions=check','--roots=shadow','--stack-trace=none','--memory='+('arc' if mode in ['arc','prepared'] else 'tracing'),'-o',binary],e);assert r.returncode==0
  row.update({'binary_sha256':sha(binary),'binary_bytes':binary.stat().st_size})
  mapping=binary.with_suffix('.map')
  if mapping.exists():
   row['map_sha256']=sha(mapping);row['map_path']=str(mapping);row['transition_table_symbols']=[line for line in mapping.read_text().splitlines() if '.transitionTable' in line]
  save()
  for stress in ([False] if mode=='baseline-tracing' else [False,True]):
   if stress:e['DARTFORGE_GC_STRESS']='1'
   r,row=run(label+'.'+mode+('.stress' if stress else '.normal'),[binary],e);row['vm_exact']=r.returncode==0 and not r.stderr and r.stdout==oracle.stdout;save();print(row['label'],row['vm_exact'],flush=True);assert row['vm_exact']
ledger['complete']=True;ledger['compiler_unchanged']=sha(compiler)==ledger['compiler_sha256'];assert ledger['compiler_unchanged'];save()

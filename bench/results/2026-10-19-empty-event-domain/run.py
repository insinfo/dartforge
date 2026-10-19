import os,json,hashlib,subprocess,pathlib,time
root=pathlib.Path('/Volumes/ssd_dock/MyRustProjects/dartforge')
out=pathlib.Path('/tmp/forge-empty-events-live');out.mkdir(exist_ok=True)
compiler=root/'target/impl-a/debug/dartforge'
sdk='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib'
env=os.environ.copy();env.update(DARTFORGE_LLVM_DIR='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64',LLVM_SYS_221_PREFIX='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64',DARTFORGE_TEST_SDK_LIB=sdk)
env['PATH']=env['DARTFORGE_LLVM_DIR']+'/bin:'+env['PATH']
for key in ['DARTFORGE_MEMORY','DARTFORGE_GC_STRESS','DARTFORGE_CLOSED_CLASS_REACHABILITY','DARTFORGE_TRACE_CLASS_REACHABILITY']:env.pop(key,None)
env.update(DARTFORGE_CLOSED_CLASS_REACHABILITY='1',DARTFORGE_TRACE_CLASS_REACHABILITY='1')
h=lambda b:hashlib.sha256(b).hexdigest()
report={'compiler_sha256':h(compiler.read_bytes()),'base_revision':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'cases':[]}
def run(name,cmd,expected=None,stress=False):
 e=env.copy()
 if stress:e['DARTFORGE_GC_STRESS']='1'
 start=time.monotonic();p=subprocess.run(list(map(str,cmd)),cwd=root,env=e,capture_output=True,timeout=600)
 (out/(name+'.stdout')).write_bytes(p.stdout);(out/(name+'.stderr')).write_bytes(p.stderr)
 report['cases'].append({'name':name,'command':list(map(str,cmd)),'gc_stress':stress,'exit':p.returncode,'seconds':time.monotonic()-start,'stdout_sha256':h(p.stdout),'stderr_sha256':h(p.stderr)})
 (out/'results.json').write_text(json.dumps(report,indent=2)+'\n')
 print(name,p.returncode,flush=True)
 assert p.returncode==0,(name,p.stderr.decode(errors='replace')[-3000:])
 if expected is not None:assert p.stdout==expected,(name,p.stdout,expected)
 return p.stdout
for case,src in [('callbacks','/tmp/forge-frame-live-smoke/callbacks.dart')]:
 source=out/(case+'.dart');source.write_bytes(pathlib.Path(src).read_bytes())
 expected=run(case+'-dart',['/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart','run',source])
 for memory in ['tracing']:
  tag=case+'-'+memory;exe=out/tag
  run(tag+'-build',[compiler,'compile-native',source,'-o',exe,'--sdk',sdk,'--optimize','--memory='+memory])
  for stress in [False,True]:
   suffix='-stress' if stress else ''
   run(tag+'-aot'+suffix,[exe],expected,stress)
   run(tag+'-jit'+suffix,[compiler,'run','--sdk',sdk,'--memory='+memory,source],expected,stress)

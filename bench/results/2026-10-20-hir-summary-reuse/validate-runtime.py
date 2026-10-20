from pathlib import Path
import os,subprocess,json,time,hashlib
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-hir-summary-reuse');artifacts=root/'target/hir-summary-validation';prefix='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64'
env=os.environ.copy();env.update(SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk',MACOSX_DEPLOYMENT_TARGET='15.0',DARTFORGE_NATIVE_CACHE=str(root/'target/rpo-native-cache'));env['PATH']=prefix+'/bin:/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin:'+env['PATH'];env.pop('DARTFORGE_GC_STRESS',None)
source=out/'ffi-callback.dart';records=[]
reference=subprocess.run(['dart',str(source)],env=env,capture_output=True,text=True);assert reference.returncode==0 and reference.stdout=='37\n32\n',reference
records.append(dict(label='dart-vm',exit=reference.returncode,stdout=reference.stdout,stderr=reference.stderr,command=['dart',str(source)]))
for label in ['before','after']:
 compiler=root/('target/test-tools/dartforge-summary-'+label);executable=artifacts/('ffi-'+label)
 command=[str(compiler),'compile-native',str(source),'--optimize','--timings','--sdk','/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib','-o',str(executable)]
 start=time.monotonic();compiled=subprocess.run(command,env=env,capture_output=True,text=True)
 records.append(dict(label=label+'-compile',command=command,exit=compiled.returncode,seconds=time.monotonic()-start,stdout=compiled.stdout,stderr=compiled.stderr,compiler_sha256=hashlib.sha256(compiler.read_bytes()).hexdigest()));(out/'runtime.json').write_text(json.dumps(records,indent=2)+'\n');print(label,'compile',compiled.returncode,flush=True)
 assert compiled.returncode==0,compiled.stderr
 for stress in [False,True]:
  runtime=env.copy()
  if stress:runtime['DARTFORGE_GC_STRESS']='1'
  result=subprocess.run([str(executable)],env=runtime,capture_output=True,text=True)
  records.append(dict(label=label+('-stress' if stress else '-normal'),command=[str(executable)],executable_sha256=hashlib.sha256(executable.read_bytes()).hexdigest(),environment={'DARTFORGE_GC_STRESS':runtime.get('DARTFORGE_GC_STRESS')},exit=result.returncode,stdout=result.stdout,stderr=result.stderr,matches_vm=result.stdout==reference.stdout));(out/'runtime.json').write_text(json.dumps(records,indent=2)+'\n');print(label,'stress',stress,result.returncode,flush=True)
  assert result.returncode==0 and result.stdout==reference.stdout and not result.stderr,records[-1]

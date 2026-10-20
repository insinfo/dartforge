from pathlib import Path
import os,subprocess,json,hashlib,time,sys
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-stack-read/runtime');artifacts=root/'target/stack-read-validation';artifacts.mkdir(exist_ok=True);source=root/'crates/emit_native/tests/fixtures/table_batch/main.dart';prefix='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64';sdk='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib'
env=os.environ.copy();env.update(DARTFORGE_LLVM_DIR=prefix,LLVM_SYS_221_PREFIX=prefix,SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk',MACOSX_DEPLOYMENT_TARGET='15.0',DARTFORGE_NATIVE_CACHE=str(root/'target/rpo-native-cache'),DARTFORGE_GENERATOR='clang',DARTFORGE_KEEP_IR='1');env['PATH']=prefix+'/bin:/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin:'+env['PATH'];env.pop('DARTFORGE_GC_STRESS',None)
label=sys.argv[1];compiler=root/('target/test-tools/dartforge-before-stack-read' if label=='before' else 'target/test-tools/dartforge-stack-read');records=[]
reference=subprocess.run(['dart',str(source)],env=env,capture_output=True,text=True);assert reference.returncode==0 and not reference.stderr,reference
records.append(dict(label='dart-vm',exit=reference.returncode,stdout=reference.stdout,stderr=reference.stderr))
def call(name,command,runtime):
 start=time.monotonic();result=subprocess.run(command,cwd=root,env=runtime,capture_output=True,text=True)
 record=dict(label=name,command=command,exit=result.returncode,seconds=time.monotonic()-start,stdout=result.stdout,stderr=result.stderr,environment={key:runtime.get(key) for key in ['DARTFORGE_GC_STRESS','DARTFORGE_NO_TABLE_PRUNING','DARTFORGE_KEEP_IR']},compiler_sha256=hashlib.sha256(compiler.read_bytes()).hexdigest());records.append(record);(out/(label+'-runtime.json')).write_text(json.dumps(records,indent=2)+'\n');print(name,result.returncode,flush=True);assert result.returncode==0,record;return result
for profile in ['production','development','unpruned']:
 exe=artifacts/(label+'-'+profile)
 command=[str(compiler),'compile-native',str(source),'--sdk',sdk,'--timings','-o',str(exe)]
 if profile!='development':command.append('--optimize')
 compile_env=env.copy()
 if profile=='unpruned':compile_env['DARTFORGE_NO_TABLE_PRUNING']='1'
 call(profile+'-compile',command,compile_env)
 for stress in [False,True]:
  runtime=env.copy()
  if stress:runtime['DARTFORGE_GC_STRESS']='1'
  result=call(profile+('-stress' if stress else '-normal'),[str(exe)],runtime)
  assert result.stdout==reference.stdout and not result.stderr,records[-1]
  records[-1]['executable_sha256']=hashlib.sha256(exe.read_bytes()).hexdigest()
  (out/(label+'-runtime.json')).write_text(json.dumps(records,indent=2)+'\n')
if label=='after':
 for stress in [False,True]:
  runtime=env.copy()
  if stress:runtime['DARTFORGE_GC_STRESS']='1'
  result=call('jit'+('-stress' if stress else '-normal'),[str(compiler),'run','--sdk',sdk,str(source)],runtime)
  assert result.stdout==reference.stdout and not result.stderr,records[-1]

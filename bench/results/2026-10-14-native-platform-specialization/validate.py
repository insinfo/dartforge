"""Validate ordinary host flags and modified SDK effects with an immutable compiler."""
from pathlib import Path
import hashlib,json,os,subprocess,sys
ROOT=Path.cwd();OUT=ROOT/'target/platform-specialization';CLI=Path(sys.argv[1]).resolve();SDK=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk');SOURCE=ROOT/'crates/emit_native/tests/fixtures/platform_flags/main.dart';CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
env=dict(os.environ,TMPDIR=str(OUT),DARTFORGE_CLANG=CLANG,LLVM_SYS_221_PREFIX=str(Path(CLANG).parent.parent))
for key in ['DARTFORGE_NO_PLATFORM_SPECIALIZATION','DARTFORGE_ARC_PREPARED','DARTFORGE_ARC_CHECK','DARTFORGE_EFFECTS','DARTFORGE_GC_STRESS']:env.pop(key,None)
vm=subprocess.run([str(SDK/'bin/dart'),'run',str(SOURCE)],capture_output=True,text=True,timeout=30);assert vm.returncode==0 and not vm.stderr
m={'compiler':str(CLI),'compiler_sha256':sha(CLI),'inputs':{str(p):sha(p)for p in SOURCE.parent.glob('*.dart')},'ordinary_VM_stdout':vm.stdout,'runs':{},'scope':'Correctness only; no startup/performance measurements. Public native SDK effect probe is a diagnostic copied SDK, not a source workaround.'}
for sdk_name,sdk,expected in [('original',SDK/'lib',vm.stdout),('wrapper',OUT/'modified-sdk/lib','platform wrapper\n'+vm.stdout)]:
 for opt in ['off','on']:
  folder=OUT/'correctness'/sdk_name/opt;folder.mkdir(parents=True,exist_ok=True);binary=folder/'program';e=dict(env,DARTFORGE_SDK_LIB=str(sdk))
  if opt=='off':e['DARTFORGE_NO_PLATFORM_SPECIALIZATION']='1'
  command=[str(CLI),'compile-native',str(SOURCE),'--sdk',str(sdk),'--optimize','--memory=tracing','--exceptions=check','--roots=shadow','--stack-trace=none','-o',str(binary)]
  compile=subprocess.run(command,capture_output=True,text=True,env=e,timeout=300)
  (folder/'compile.stdout').write_text(compile.stdout);(folder/'compile.stderr').write_text(compile.stderr)
  item={'command':command,'compile_exit':compile.returncode,'opt_out':e.get('DARTFORGE_NO_PLATFORM_SPECIALIZATION'),'DARTFORGE_SDK_LIB':e['DARTFORGE_SDK_LIB']}
  if compile.returncode==0:
   run=subprocess.run([str(binary)],capture_output=True,text=True,env=e,timeout=30)
   (folder/'run.stdout').write_text(run.stdout);(folder/'run.stderr').write_text(run.stderr)
   item.update(binary_sha256=sha(binary),exit=run.returncode,stdout=run.stdout,stderr=run.stderr,exact_expected=run.returncode==0 and run.stdout==expected and not run.stderr)
  m['runs'][sdk_name+'.'+opt]=item;(OUT/'validation.json').write_text(json.dumps(m,indent=2)+'\n');print(sdk_name,opt,item,flush=True)
  assert item.get('exact_expected'),item
assert sha(CLI)==m['compiler_sha256'] and all(sha(p)==h for p,h in m['inputs'].items());m['inputs_stable']=True;(OUT/'validation.json').write_text(json.dumps(m,indent=2)+'\n')

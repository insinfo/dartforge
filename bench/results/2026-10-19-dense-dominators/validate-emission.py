import os,sys,json,subprocess,hashlib,time
from pathlib import Path
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=root/'target/dense-dominator-validation'
label=sys.argv[1];compiler=Path(sys.argv[2]);prefix='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64'
env=os.environ.copy();env['PATH']=prefix+'/bin:/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin:'+env['PATH'];env['SDKROOT']='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk';env['MACOSX_DEPLOYMENT_TARGET']='15.0'
command=[str(compiler),'compile-native',str(out/'branches.dart'),'--emit-ir','--digest','--timings','--sdk','/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib','-o',str(out/(label+'.ll'))]
start=time.monotonic();result=subprocess.run(command,cwd=root,env=env,capture_output=True);elapsed=time.monotonic()-start
(out/(label+'.stdout')).write_bytes(result.stdout);(out/(label+'.stderr')).write_bytes(result.stderr)
record={'command':command,'exit':result.returncode,'seconds':elapsed,'compiler_sha256':hashlib.sha256(compiler.read_bytes()).hexdigest(),'source_sha256':hashlib.sha256((out/'branches.dart').read_bytes()).hexdigest(),'ir_sha256':hashlib.sha256((out/(label+'.ll')).read_bytes()).hexdigest() if result.returncode==0 else None}
(out/(label+'.json')).write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(record),flush=True);assert result.returncode==0

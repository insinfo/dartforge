from pathlib import Path
import os,subprocess,json,hashlib,time,sys
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-workspace-format/runtime');artifacts=root/'target/workspace-format-validation';artifacts.mkdir(exist_ok=True);source=root/'crates/emit_native/tests/fixtures/table_batch/main.dart';prefix='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64';sdk='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib'
env=os.environ.copy();env.update(DARTFORGE_LLVM_DIR=prefix,LLVM_SYS_221_PREFIX=prefix,SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk',MACOSX_DEPLOYMENT_TARGET='15.0',DARTFORGE_NATIVE_CACHE=str(root/'target/rpo-native-cache'),DARTFORGE_GENERATOR='clang',DARTFORGE_KEEP_IR='1');env['PATH']=prefix+'/bin:/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin:'+env['PATH'];env.pop('DARTFORGE_GC_STRESS',None)

compiler=root/'target/test-tools/dartforge-workspace-format';records=[]
for target in ['windows-x64','linux-arm64']:
 binary=artifacts/('cross-'+target+('.exe' if target.startswith('windows') else ''))
 command=[str(compiler),'compile-native',str(source),'--sdk',sdk,'--target',target,'--target-bundle',str(root/'target/cross-bundles/workspace-format'/target),'--optimize','-o',str(binary)]
 start=time.monotonic();result=subprocess.run(command,cwd=root,env=env,capture_output=True,text=True);record=dict(target=target,command=command,exit=result.returncode,seconds=time.monotonic()-start,stdout=result.stdout,stderr=result.stderr);records.append(record)
 if result.returncode==0:record.update(binary=str(binary),sha256=hashlib.sha256(binary.read_bytes()).hexdigest())
 (out/'cross-builds.json').write_text(json.dumps(records,indent=2)+'\n');print(target,result.returncode,flush=True);assert result.returncode==0,record

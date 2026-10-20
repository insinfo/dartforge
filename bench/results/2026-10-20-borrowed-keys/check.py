import os,subprocess,json
from pathlib import Path
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-borrowed-keys/checks');out.mkdir(parents=True,exist_ok=True)
env=os.environ.copy();prefix='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64'
env.update(DARTFORGE_LLVM_DIR=prefix,LLVM_SYS_221_PREFIX=prefix,SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk',MACOSX_DEPLOYMENT_TARGET='15.0',DARTFORGE_SDK_LIB='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib',DARTFORGE_TEST_LINUX_SYSROOT=str(root/'target/cross-bundles/current/linux-arm64/sysroot'),RUSTDOCFLAGS='-D warnings',CARGO_TARGET_DIR=str(root/'target/string-length-validation'))
env['PATH']=prefix+'/bin:/Volumes/ssd_dock/tools/rea/node-v24.21.0-darwin-arm64/bin:/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin:'+env['PATH']
commands=[('directed',['cargo','test','--locked','-p','dartforge-emit-native','--lib','stream_verify','--','--include-ignored','--nocapture']),('clippy',['cargo','clippy','--locked','--workspace','--all-targets','--','-D','warnings']),('feature-clippy',['cargo','clippy','--locked','-p','dartforge-emit-native','--features','embedded-llvm','--all-targets','--','-D','warnings']),('doc',['cargo','doc','--locked','--workspace','--no-deps','--document-private-items']),('compiler',['cargo','build','--locked','-p','dartforge-cli','--features','jit']),('native-lib',['cargo','test','--locked','-p','dartforge-emit-native','--lib','--','--include-ignored'])]
results=[]
for name,command in commands:
 with (out/(name+'.log')).open('w') as log:
  result=subprocess.run(command,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
 results.append(dict(name=name,command=command,exit=result.returncode));(out/'checks.json').write_text(json.dumps(results,indent=2)+'\n');print(name,result.returncode,flush=True)
 if result.returncode: break

if len(results)==len(commands) and all(row['exit']==0 for row in results):
 import shutil
 destination=root/'target/test-tools/dartforge-borrowed-keys'
 temporary=destination.with_suffix('.publish.tmp')
 shutil.copy2(root/'target/string-length-validation/debug/dartforge',temporary)
 os.replace(temporary,destination)

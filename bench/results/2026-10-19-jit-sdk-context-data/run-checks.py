import os,subprocess,json
from pathlib import Path
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-jit-sdk-context')
env=os.environ.copy();prefix='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64'
env.update(DARTFORGE_LLVM_DIR=prefix,LLVM_SYS_221_PREFIX=prefix,SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk',MACOSX_DEPLOYMENT_TARGET='15.0',DARTFORGE_SDK_LIB='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib',DARTFORGE_DART_SDK='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk',DARTFORGE_NATIVE_CACHE=str(root/'target/string-length-native-cache'),DARTFORGE_SDK_DLL=str(root/'target/string-length-native-cache/sdk/6b6831042293752baa4cd97534016ec3/libdfsdk_6b6831042293752b.dylib'),RUSTDOCFLAGS='-D warnings')
env['PATH']=prefix+'/bin:/Volumes/ssd_dock/tools/rea/node-v24.21.0-darwin-arm64/bin:/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin:'+env['PATH']
commands=[('jit',['cargo','test','--locked','-p','dartforge-jit','--','--include-ignored']),('clippy',['cargo','clippy','--locked','--workspace','--all-targets','--','-D','warnings']),('doc',['cargo','doc','--locked','--workspace','--no-deps','--document-private-items'])]
results=[]
for name,command in commands:
 with (out/(name+'.log')).open('w') as log:result=subprocess.run(command,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
 results.append(dict(name=name,command=command,exit=result.returncode));(out/'checks.json').write_text(json.dumps(results,indent=2)+'\n');print(name,result.returncode,flush=True)
 if result.returncode:break

from pathlib import Path
import os,subprocess,json
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=root/'target/current-desktop-validation';prefix='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64'
env=os.environ.copy();env.update(DARTFORGE_LLVM_DIR=prefix,LLVM_SYS_221_PREFIX=prefix,SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk',MACOSX_DEPLOYMENT_TARGET='15.0',DARTFORGE_SDK_LIB='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib',DARTFORGE_NATIVE_CACHE=str(root/'target/rpo-native-cache'),CARGO_TARGET_DIR=str(root/'target/string-length-validation'),CARGO_BUILD_JOBS='2',RUST_TEST_THREADS='2',RUSTDOCFLAGS='-D warnings');env['PATH']=prefix+'/bin:/Volumes/ssd_dock/tools/rea/node-v24.21.0-darwin-arm64/bin:/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin:'+env['PATH']
commands=[('clippy',['cargo','clippy','--locked','--workspace','--all-targets','--','-D','warnings']),('feature-clippy',['cargo','clippy','--locked','-p','dartforge-cli','--features','native,jit','--all-targets','--','-D','warnings']),('doc',['cargo','doc','--locked','--workspace','--no-deps','--document-private-items']),('cli',['cargo','test','--locked','-p','dartforge-cli','--features','native,jit','--no-fail-fast','--','--include-ignored']),('fmt',['cargo','fmt','--all','--','--check'])]
records=[]
for name,command in commands:
 print('Starting',name,flush=True)
 with (out/('final-'+name+'.log')).open('w') as log:p=subprocess.run(command,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
 records.append(dict(name=name,command=command,exit=p.returncode));(out/'final-checks.json').write_text(json.dumps(records,indent=2)+'\n');print(name,p.returncode,flush=True)
 if p.returncode:break

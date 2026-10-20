import os,subprocess,json
from pathlib import Path
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-macho-checks')
env=os.environ.copy();prefix='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64'
env.update(DARTFORGE_LLVM_DIR=prefix,LLVM_SYS_221_PREFIX=prefix,SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk',MACOSX_DEPLOYMENT_TARGET='15.0',DARTFORGE_SDK_LIB='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib',RUSTDOCFLAGS='-D warnings',CARGO_TARGET_DIR=str(root/'target/string-length-validation'))
env['PATH']=prefix+'/bin:/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin:'+env['PATH']
command=['cargo','test','--locked','-p','dartforge-emit-native','--doc','linked_images::macho_relocations']
with (out/'doc-test.log').open('w') as log:
 result=subprocess.run(command,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
(out/'doc-test.json').write_text(json.dumps(dict(command=command,exit=result.returncode),indent=2)+'\n');print('doc-test',result.returncode,flush=True)

import os,subprocess,json
from pathlib import Path
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-workspace-format/checks');out.mkdir(parents=True,exist_ok=True)
env=os.environ.copy();prefix='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64'
env.update(DARTFORGE_LLVM_DIR=prefix,LLVM_SYS_221_PREFIX=prefix,SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk',MACOSX_DEPLOYMENT_TARGET='15.0',DARTFORGE_SDK_LIB='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib',DARTFORGE_TEST_LINUX_SYSROOT=str(root/'target/cross-bundles/current/linux-arm64/sysroot'),RUSTDOCFLAGS='-D warnings',CARGO_TARGET_DIR=str(root/'target/string-length-validation'))
env['PATH']=prefix+'/bin:/Volumes/ssd_dock/tools/rea/node-v24.21.0-darwin-arm64/bin:/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin:'+env['PATH']

env.update(CARGO_BUILD_JOBS='2',RUST_TEST_THREADS='2',DARTFORGE_NATIVE_CACHE=str(root/'target/rpo-native-cache'),DARTFORGE_TEST_SDK_LIB=env['DARTFORGE_SDK_LIB'])

import shutil,hashlib
command=['cargo','build','--locked','-p','dartforge-cli','--features','jit']
with (out/'compiler.log').open('w') as log: result=subprocess.run(command,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
metadata=dict(command=command,exit=result.returncode)
if result.returncode==0:
 destination=root/'target/test-tools/dartforge-workspace-format';temporary=destination.with_suffix('.publish.tmp');shutil.copy2(root/'target/string-length-validation/debug/dartforge',temporary);os.replace(temporary,destination);metadata.update(binary=str(destination),sha256=hashlib.sha256(destination.read_bytes()).hexdigest())
(out/'compiler.json').write_text(json.dumps(metadata,indent=2)+'\n');print(json.dumps(metadata),flush=True)

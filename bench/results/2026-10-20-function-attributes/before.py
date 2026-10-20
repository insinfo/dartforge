import os,subprocess,json
from pathlib import Path
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-function-attributes/checks');out.mkdir(parents=True,exist_ok=True)
env=os.environ.copy();prefix='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64'
env.update(DARTFORGE_LLVM_DIR=prefix,LLVM_SYS_221_PREFIX=prefix,SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk',MACOSX_DEPLOYMENT_TARGET='15.0',DARTFORGE_SDK_LIB='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib',DARTFORGE_TEST_LINUX_SYSROOT=str(root/'target/cross-bundles/current/linux-arm64/sysroot'),RUSTDOCFLAGS='-D warnings',CARGO_TARGET_DIR=str(root/'target/string-length-validation'))
env['PATH']=prefix+'/bin:/Volumes/ssd_dock/tools/rea/node-v24.21.0-darwin-arm64/bin:/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin:'+env['PATH']

source=root/'crates/emit_native/src/sdk_module.rs'
original=source.read_bytes()
previous=subprocess.check_output(['git','show','5838ac3bc11617bc5957b1795d21b9a20e206f07:crates/emit_native/src/sdk_module.rs'],cwd=root).decode().replace('mod context_calls;', 'mod context_calls;\nmod function_attributes;')
try:
 source.write_text(previous)
 with (out/'before-directed.log').open('w') as log:
  r=subprocess.run(['cargo','test','--locked','-p','dartforge-emit-native','--lib','function_attributes','--','--include-ignored'],cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
 print('before-directed',r.returncode,flush=True)
finally:
 source.write_bytes(original)

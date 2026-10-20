from pathlib import Path
import os,subprocess,json,sys
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=root/'target/current-desktop-validation';label=sys.argv[1]
env=os.environ.copy();prefix='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64';env.update(DARTFORGE_LLVM_DIR=prefix,LLVM_SYS_221_PREFIX=prefix,SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk',MACOSX_DEPLOYMENT_TARGET='15.0',DARTFORGE_SDK_LIB='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib',DARTFORGE_NATIVE_CACHE=str(root/'target/rpo-native-cache'),CARGO_TARGET_DIR=str(root/'target/string-length-validation'),CARGO_BUILD_JOBS='2',RUST_TEST_THREADS='2');env['PATH']=prefix+'/bin:'+env['PATH']
command=['cargo','test','--locked','-p','dartforge-cli','--features','native,jit','--test','jit_program','explicit_sdk_selects_the_jit_library_in_run_reload_and_restart','--','--include-ignored']
with (out/(label+'.log')).open('w') as log:r=subprocess.run(command,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
(out/(label+'.json')).write_text(json.dumps(dict(command=command,exit=r.returncode),indent=2)+'\n');print(label,r.returncode,flush=True)

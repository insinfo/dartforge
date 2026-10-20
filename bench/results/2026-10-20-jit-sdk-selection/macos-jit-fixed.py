from pathlib import Path
import os,subprocess,json
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=root/'target/current-desktop-validation';prefix='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64';poison=out/'invalid-sdk';poison.mkdir(exist_ok=True);(poison/'libraries.json').write_text('not a library manifest')
env=os.environ.copy();env.update(SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk',MACOSX_DEPLOYMENT_TARGET='15.0',DARTFORGE_LLVM_DIR=prefix,DARTFORGE_GENERATOR='clang',DARTFORGE_NATIVE_CACHE=str(root/'target/rpo-native-cache'),DARTFORGE_SDK_LIB=str(poison));env['PATH']=prefix+'/bin:'+env['PATH']
for key in ['DART_SDK','DARTFORGE_SDK_DLL','DARTFORGE_GC_STRESS']:env.pop(key,None)
command=['python3',str(root/'scripts/validate-native-desktop.py'),'--compiler',str(root/'target/test-tools/dartforge-explicit-sdk'),'--ui-root','/Volumes/ssd_dock/MyProjectsDart/dart_ui','--sdk','/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib','--out',str(out/'macos-jit-fixed'),'--mode','jit']
(out/'macos-jit-fixed-command.json').write_text(json.dumps(dict(command=command,ambient_sdk=str(poison),explicit_sdk='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib'),indent=2)+'\n')
with (out/'macos-jit-fixed.log').open('w') as log:r=subprocess.run(command,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
print('fixed macOS JIT',r.returncode);raise SystemExit(r.returncode)

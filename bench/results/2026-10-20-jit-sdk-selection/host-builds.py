from pathlib import Path
import os,subprocess,json,hashlib,time
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');ui=Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui');out=root/'target/current-desktop-validation';out.mkdir(exist_ok=True)
compiler=root/'target/test-tools/dartforge-root-bit-iteration';sdk='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib';prefix='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64'
env=os.environ.copy();env.update(SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk',MACOSX_DEPLOYMENT_TARGET='15.0',DARTFORGE_LLVM_DIR=prefix,DARTFORGE_GENERATOR='clang',DARTFORGE_NATIVE_CACHE=str(root/'target/rpo-native-cache'));env['PATH']=prefix+'/bin:'+env['PATH']
for key in ['DARTFORGE_CLOSED_CLASS_REACHABILITY','DARTFORGE_GC_STRESS','DARTFORGE_NO_PRUNING','DARTFORGE_NO_TABLE_PRUNING']:env.pop(key,None)
rows=[]
def run(name,command):
 start=time.monotonic()
 with (out/(name+'.log')).open('w') as log:
  p=subprocess.Popen(command,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
  (out/'state.json').write_text(json.dumps(dict(status='running',name=name,pid=p.pid,command=command),indent=2)+'\n');code=p.wait()
 row=dict(name=name,command=command,exit=code,seconds=time.monotonic()-start,compiler_sha256=hashlib.sha256(compiler.read_bytes()).hexdigest());rows.append(row);(out/'builds.json').write_text(json.dumps(rows,indent=2)+'\n');print(name,code,round(row['seconds'],2),flush=True);assert code==0,row
run('macos-ui',['python3',str(root/'scripts/validate-native-desktop.py'),'--compiler',str(compiler),'--ui-root',str(ui),'--sdk',sdk,'--out',str(out/'macos-ui'),'--mode','all'])
for target in ['linux-arm64','windows-x64']:
 for kind,entry in [('explorer','bin/file_explorer.dart'),('context','tool/context_menu_retarget_smoke.dart')]:
  exe=out/(target+'-'+kind+('.exe' if target=='windows-x64' else ''));bundle=root/'target/cross-bundles/current'/target
  command=[str(compiler),'compile-native',str(ui/'examples/file_explorer'/entry),'--optimize','--sdk',sdk,'--packages',str(ui/'examples/file_explorer/.dart_tool/package_config.json'),'--target',target,'--target-bundle',str(bundle),'-o',str(exe)]
  run(target+'-'+kind,command);rows[-1].update(executable_sha256=hashlib.sha256(exe.read_bytes()).hexdigest(),bytes=exe.stat().st_size);(out/'builds.json').write_text(json.dumps(rows,indent=2)+'\n')
(out/'state.json').write_text(json.dumps(dict(status='finished',runs=len(rows)),indent=2)+'\n')

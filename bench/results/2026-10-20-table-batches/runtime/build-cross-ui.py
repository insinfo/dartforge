from pathlib import Path
import os,subprocess,json,time,hashlib
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-batch-tables-runtime');artifacts=root/'target/batch-tables-validation';ui=Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui');sdk='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib';prefix='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64';env=os.environ.copy();env.update(SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk',MACOSX_DEPLOYMENT_TARGET='15.0',DARTFORGE_LLVM_DIR=prefix,DARTFORGE_GENERATOR='clang',DARTFORGE_NATIVE_CACHE=str(root/'target/rpo-native-cache'));env['PATH']=prefix+'/bin:'+env['PATH'];records=[]
for target in ['linux-arm64','windows-x64']:
 for label in ['before','after']:
  compiler=root/('target/test-tools/dartforge-proof-views' if label=='before' else 'target/test-tools/dartforge-batch-tables');bundle=root/('target/cross-bundles/sdk-image-owner' if label=='before' else 'target/cross-bundles/batch-tables')/target
  for kind in (['explorer'] if label=='before' else ['explorer','context']):
   name=f'{target}-{label}-{kind}';exe=artifacts/(name+('.exe' if target=='windows-x64' else ''));source=ui/('examples/file_explorer/bin/file_explorer.dart' if kind=='explorer' else 'examples/file_explorer/tool/context_menu_retarget_smoke.dart')
   command=[str(compiler),'compile-native',str(source),'--optimize','--sdk',sdk,'--packages',str(ui/'examples/file_explorer/.dart_tool/package_config.json'),'--target',target,'--target-bundle',str(bundle),'-o',str(exe)]
   start=time.monotonic()
   with (out/(name+'.stdout')).open('w') as stdout,(out/(name+'.stderr')).open('w') as stderr:p=subprocess.run(command,env=env,cwd=root,stdout=stdout,stderr=stderr)
   row=dict(name=name,command=command,exit=p.returncode,seconds=time.monotonic()-start,compiler_sha256=hashlib.sha256(compiler.read_bytes()).hexdigest(),bytes=exe.stat().st_size if p.returncode==0 else None,sha256=hashlib.sha256(exe.read_bytes()).hexdigest() if p.returncode==0 else None);records.append(row);(out/'cross-ui-builds.json').write_text(json.dumps(records,indent=2)+'\n');print(name,p.returncode,round(row['seconds'],2),row['bytes'],flush=True);assert p.returncode==0,row

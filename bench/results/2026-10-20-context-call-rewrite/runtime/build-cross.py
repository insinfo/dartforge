from pathlib import Path
import os,subprocess,json,time,hashlib,sys
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-context-footprints/runtime');artifacts=root/'target/context-calls-validation';artifacts.mkdir(exist_ok=True);ui=Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui');sdk='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib';prefix='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64';target=sys.argv[1]
compiler=root/'target/test-tools/dartforge-context-calls';bundle=root/'target/cross-bundles/current'/target
env=os.environ.copy();env.update(SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk',MACOSX_DEPLOYMENT_TARGET='15.0',DARTFORGE_LLVM_DIR=prefix,DARTFORGE_GENERATOR='clang',DARTFORGE_NATIVE_CACHE=str(root/'target/rpo-native-cache'),DARTFORGE_KEEP_IR='1',DARTFORGE_PARALLEL_PARTS='1');env['PATH']=prefix+'/bin:'+env['PATH'];records=[]
for kind,source in [('fixture',root/'crates/emit_native/tests/fixtures/context_literal/main.dart')]:
 name=f'{target}-{kind}';exe=artifacts/(name+('.exe' if target=='windows-x64' else ''));command=[str(compiler),'compile-native',str(source),'--optimize','--sdk',sdk,'--target',target,'--target-bundle',str(bundle),'-o',str(exe)]
 if kind!='fixture':command+=['--packages',str(ui/'examples/file_explorer/.dart_tool/package_config.json')]
 start=time.monotonic()
 with (out/(name+'.stdout')).open('w') as stdout,(out/(name+'.stderr')).open('w') as stderr:p=subprocess.run(command,env=env,cwd=root,stdout=stdout,stderr=stderr)
 row=dict(name=name,command=command,exit=p.returncode,seconds=time.monotonic()-start,compiler_sha256=hashlib.sha256(compiler.read_bytes()).hexdigest(),bytes=exe.stat().st_size if p.returncode==0 else None,sha256=hashlib.sha256(exe.read_bytes()).hexdigest() if p.returncode==0 else None,environment={key:env[key] for key in ['DARTFORGE_KEEP_IR','DARTFORGE_PARALLEL_PARTS']});records.append(row);(out/f'{target}-builds.json').write_text(json.dumps(records,indent=2)+'\n');print(name,p.returncode,round(row['seconds'],2),row['bytes'],flush=True);assert p.returncode==0,row

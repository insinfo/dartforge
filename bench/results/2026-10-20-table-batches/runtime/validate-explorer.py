from pathlib import Path
import os,subprocess,time,json,hashlib,sys,re
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-batch-tables-runtime');artifacts=root/'target/batch-tables-validation';prefix='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64'
env=os.environ.copy();env.update(SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk',MACOSX_DEPLOYMENT_TARGET='15.0',DARTFORGE_NATIVE_CACHE=str(root/'target/rpo-native-cache'));env['PATH']=prefix+'/bin:/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin:'+env['PATH']
compilers={'before':root/'target/test-tools/dartforge-proof-views','after':root/'target/test-tools/dartforge-batch-tables'}
def digest(path):
 h=hashlib.sha256()
 with path.open('rb') as stream:
  for block in iter(lambda:stream.read(1<<20),b''):h.update(block)
 return h.hexdigest()
expected=None
phase=sys.argv[1];records=[]
sequence=[('before','explorer-before'),('after','explorer-after')]
for label,name in sequence:
 ir=artifacts/(label+'.ll');command=[str(compilers[label]),'compile-native','/Volumes/ssd_dock/MyProjectsDart/dart_ui/examples/file_explorer/bin/file_explorer.dart','--optimize','--emit-ir','--digest','--timings','--packages','/Volumes/ssd_dock/MyProjectsDart/dart_ui/examples/file_explorer/.dart_tool/package_config.json','--sdk','/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib','-o',str(ir)]
 start=time.monotonic()
 with (out/(name+'.stdout')).open('w') as stdout,(out/(name+'.stderr')).open('w') as stderr:
  result=subprocess.run(command,cwd=root,env=env,stdout=stdout,stderr=stderr)
 elapsed=time.monotonic()-start;text=(out/(name+'.stderr')).read_text();rounds=re.findall(r'(\d+) reachability rounds',text)
 record=dict(label=label,name=name,command=command,exit=result.returncode,seconds=elapsed,compiler_sha256=digest(compilers[label]),ir_sha256=digest(ir) if result.returncode==0 else None,ir_bytes=ir.stat().st_size if result.returncode==0 else None,reachability_rounds=int(rounds[0]) if rounds else None,stderr=text)
 records.append(record);(out/(phase+'-comparison.json')).write_text(json.dumps(records,indent=2)+'\n');print(name,result.returncode,round(elapsed,3),'rounds',record['reachability_rounds'],flush=True)
 assert result.returncode==0,record

 if label=='after':assert record['reachability_rounds']>=2

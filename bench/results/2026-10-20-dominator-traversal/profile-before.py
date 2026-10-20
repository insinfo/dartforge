import os,subprocess,time,json,hashlib
from pathlib import Path
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-rpo-validation');compiler=root/'target/test-tools/dartforge-rpo-before';prefix='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64'
env=os.environ.copy();env.update(SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk',MACOSX_DEPLOYMENT_TARGET='15.0',DARTFORGE_NATIVE_CACHE=str(root/'target/rpo-native-cache'));env['PATH']=prefix+'/bin:/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin:'+env['PATH']
entry='/Volumes/ssd_dock/MyProjectsDart/dart_ui/examples/file_explorer/bin/file_explorer.dart';packages='/Volumes/ssd_dock/MyProjectsDart/dart_ui/examples/file_explorer/.dart_tool/package_config.json'
command=[str(compiler),'compile-native',entry,'--optimize','--emit-ir','--digest','--timings','--packages',packages,'--sdk','/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib','-o',str(out/'explorer-before.ll')]
records=[]
for warm in [False,True]:
 label='warm-before' if warm else 'cold-before';start=time.monotonic()
 with (out/(label+'.stdout')).open('w') as stdout,(out/(label+'.stderr')).open('w') as stderr:
  process=subprocess.Popen(command,cwd=root,env=env,stdout=stdout,stderr=stderr)
  time.sleep(0.5)
  sampling={'live_before_sample':process.poll() is None}
  if sampling['live_before_sample']:
   sampled=subprocess.run(['sample',str(process.pid),'2','-file',str(out/(label+'.sample.txt'))],capture_output=True,text=True);sampling.update(exit=sampled.returncode,stdout=sampled.stdout,stderr=sampled.stderr)
  code=process.wait()
 record=dict(label=label,command=command,pid=process.pid,exit=code,seconds=time.monotonic()-start,sampling=sampling,compiler_sha256=hashlib.sha256(compiler.read_bytes()).hexdigest())
 if code==0:record['ir_sha256']=hashlib.sha256((out/'explorer-before.ll').read_bytes()).hexdigest()
 records.append(record);(out/'profile-before.json').write_text(json.dumps(records,indent=2)+'\n');print(label,code,round(record['seconds'],3),flush=True)
 if code:break

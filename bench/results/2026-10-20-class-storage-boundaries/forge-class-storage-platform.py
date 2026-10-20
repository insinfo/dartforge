import os,subprocess,json,hashlib
from pathlib import Path
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge'); out=Path('/tmp/forge-class-storage-platform');out.mkdir(exist_ok=True)
env=os.environ.copy();env['PATH']='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin:/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin:'+env['PATH'];env['DARTFORGE_SDK_LIB']='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib';env['DARTFORGE_NATIVE_CACHE']=str(root/'target/string-length-native-cache')
compiler=root/'target/string-length-validation/debug/dartforge'
records=[]
for closed in [False,True]:
 name='closed' if closed else 'default'; opts=env.copy()
 if closed: opts.update(DARTFORGE_CLOSED_CLASS_REACHABILITY='1',DARTFORGE_TRACE_CLASS_REACHABILITY='1')
 command=[str(compiler),'compile-native','crates/emit_native/tests/fixtures/constant_args/main.dart','--optimize','--emit-ir','-o',str(out/(name+'.ll'))]
 with (out/(name+'.stdout')).open('w') as stdout,(out/(name+'.stderr')).open('w') as stderr:
  result=subprocess.run(command,cwd=root,env=opts,stdout=stdout,stderr=stderr)
 text=(out/(name+'.ll')).read_text() if result.returncode==0 else ''
 definitions=[line for line in text.splitlines() if line.startswith('define ') and ('WindowsBackend' in line or 'LinuxBackend' in line)]
 records.append(dict(name=name,command=command,exit=result.returncode,foreign_backend_definitions=definitions,compiler_sha256=hashlib.sha256(compiler.read_bytes()).hexdigest()))
 (out/'results.json').write_text(json.dumps(records,indent=2)+'\n'); print(name,result.returncode,len(definitions),flush=True)

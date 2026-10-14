import runpy, os, json, hashlib, pathlib, subprocess
root=pathlib.Path.cwd(); out=root/'target/lsp-shorthand'; sdk=pathlib.Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk')
Session=runpy.run_path(str(root/'scripts/verify-lsp-analyzer.py'))['Session']
source="class C { final int value; C(this.value); }\nint read(C c) { if (c case C(:final value)) { return value; } return 0; }\n"
(out/'pubspec.yaml').write_text('name: shorthand_probe\nenvironment:\n  sdk: ">=3.6.2 <4.0.0"\n'); path=out/'main.dart'; path.write_text(source)
wrapper=out/'official-lsp'; wrapper.write_text('#!/bin/sh\nexec '+str(sdk/'bin/dart')+' language-server --protocol=lsp\n'); wrapper.chmod(0o755)
env=os.environ.copy(); env['DART_SDK']=str(sdk); env['DARTFORGE_SDK_LIB']=str(sdk/'lib')
results={}
for label,binary in [('forge',root/'target/p4/debug/dartforge-lsp'),('official',wrapper)]:
 with (out/(label+'.stderr')).open('w') as log:
  session=Session(binary,env,log)
  try:
   session.request('initialize',{'processId':os.getpid(),'rootUri':out.as_uri(),'capabilities':{}}); session.send('initialized',{})
   session.send('textDocument/didOpen',{'textDocument':{'uri':path.as_uri(),'languageId':'dart','version':1,'text':source}})
   response=session.request('textDocument/rename',{'textDocument':{'uri':path.as_uri()},'position':{'line':1,'character':source.splitlines()[1].index('return value')+8},'newName':'item'})
   results[label]={'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'rename':response}
  finally:
   session.close(); (out/(label+'.transcript.json')).write_text(json.dumps(session.transcript,indent=2))
(out/'repro.json').write_text(json.dumps(results,indent=2)); print(json.dumps(results,indent=2))

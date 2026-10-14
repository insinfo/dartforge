import runpy, os, pathlib, hashlib, json, shutil, subprocess
root=pathlib.Path.cwd(); out=root/'target/lsp-shorthand/final';out.mkdir(exist_ok=True)
Session=runpy.run_path(str(root/'scripts/verify-lsp-analyzer.py'))['Session'];sdk=pathlib.Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk');env=os.environ.copy();env['DART_SDK']=str(sdk);env['DARTFORGE_SDK_LIB']=str(sdk/'lib')
wrapper=out/'official-lsp';wrapper.write_text('#!/bin/sh\nexec '+str(sdk/'bin/dart')+' language-server --protocol=lsp\n');wrapper.chmod(0o755)
(out/'pubspec.yaml').write_text('name: shorthand_probe\nenvironment:\n  sdk: ">=3.6.2 <4.0.0"\n')
base='class C { final int value; C(this.value); }\n'
cases={name:base+'int read(C c) { if (c case C('+pat+')) { return value; } return 0; }\n' for name,pat in [('var',':var value'),('final',':final value'),('typed_final',':final int value'),('typed',':int value'),('comments',': /* colon: marker */ final value'),('explicit','value:final value')]}
cases['record']='int read(({int value}) record) { final (:value) = record; return value; }\n'
cases['nested']=base+'class Outer { C get inner => C(1); }\nint read(Outer o) { if (o case Outer(inner:C(:final value))) { return value; } return 0; }\n'
results={}; path=out/'main.dart'; path.write_text(cases['var']);uri=path.as_uri()
def position(source,offset):
 prefix=source[:offset];return {'line':prefix.count('\n'),'character':len(prefix.rsplit('\n',1)[-1].encode('utf-16-le'))//2}
def apply(source,reply):
 changes=reply.get('changes',{}).get(uri,[])
 def offset(p):
  lines=source.splitlines(keepends=True);return len(''.join(lines[:p['line']]))+p['character']
 for edit in sorted(changes,key=lambda e:offset(e['range']['start']),reverse=True):
  a,b=offset(edit['range']['start']),offset(edit['range']['end']);source=source[:a]+edit['newText']+source[b:]
 return source
for label,binary in [('baseline',root/'target/lsp-shorthand/lsp-before'),('fixed',root/'target/p4/debug/dartforge-lsp'),('official',wrapper)]:
 with (out/(label+'.stderr')).open('w') as log:
  session=Session(binary,env,log); results[label]={}
  try:
   session.request('initialize',{'processId':os.getpid(),'rootUri':out.as_uri(),'capabilities':{}});session.send('initialized',{})
   for version,(name,source) in enumerate(cases.items(),1):
    if version==1:session.send('textDocument/didOpen',{'textDocument':{'uri':uri,'languageId':'dart','version':1,'text':source}})
    else:session.send('textDocument/didChange',{'textDocument':{'uri':uri,'version':version},'contentChanges':[{'text':source}]})
    reply=session.request('textDocument/rename',{'textDocument':{'uri':uri},'position':position(source,source.rfind('return value')+8),'newName':'item'})
    results[label][name]={'reply':reply,'text':apply(source,reply),'source_sha256':hashlib.sha256(source.encode()).hexdigest()}
  finally:session.close();(out/(label+'.transcript.json')).write_text(json.dumps(session.transcript,indent=2))
for name in cases:
 assert results['fixed'][name]['text']==results['official'][name]['text'],name
results['metadata']={'sdk':'3.6.2','binary_sha256':{label:hashlib.sha256(binary.read_bytes()).hexdigest() for label,binary in [('baseline',root/'target/lsp-shorthand/lsp-before'),('fixed',root/'target/p4/debug/dartforge-lsp'),('dart',sdk/'bin/dart')]},'sources':{path:hashlib.sha256((root/path).read_bytes()).hexdigest() for path in ['crates/lsp/src/rename.rs','crates/lsp/tests/rename_patterns.rs']},'comparison':'applied WorkspaceEdit text (edit ordering is not required equal)','cases':len(cases),'fixed_matches_official':True,'baseline_mismatches':[name for name in cases if results['baseline'][name]['text']!=results['official'][name]['text']]}
(out/'validation.json').write_text(json.dumps(results,indent=2)); print(json.dumps(results['metadata'],indent=2))

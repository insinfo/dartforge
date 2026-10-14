import runpy, os, json, hashlib, pathlib
root=pathlib.Path.cwd(); out=root/'target/lsp-field-pattern/final'; sdk=pathlib.Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk')
Session=runpy.run_path(str(root/'scripts/verify-lsp-analyzer.py'))['Session']
(out/'pubspec.yaml').write_text('name: field_pattern_probe\nenvironment:\n  sdk: ">=3.6.2 <4.0.0"\n')
wrapper=out/'official-lsp'; wrapper.write_text('#!/bin/sh\nexec '+str(sdk/'bin/dart')+' language-server --protocol=lsp\n'); wrapper.chmod(0o755)
cases=[]
for pattern in [':var value', ':final value', ':final int value', ':int value', ': /* colon: keep */ final value','value:final value']:
 cases.append(('field-'+str(len(cases)), 'class C { final int value; C(this.value); }\nint read(C c) { if (c case C('+pattern+')) { return value; } return 0; }\n','item'))
cases += [('inherited','class Base { int get value => 1; }\nclass C extends Base {}\nclass Outer { C get inner => C(); }\nint read(Outer o) { if (o case Outer(inner:C(:final value))) { return value; } return 0; }\n','item'),('private','class C { int get value => 1; }\nint read(C c) { if (c case C(:final value)) { return value; } return 0; }\n','_item')]
env=os.environ.copy();env['DART_SDK']=str(sdk);env['DARTFORGE_SDK_LIB']=str(sdk/'lib')
def offset(source,pos):
 lines=source.splitlines(keepends=True);return sum(map(len,lines[:pos['line']]))+len(lines[pos['line']].encode('utf-16-le')[:pos['character']*2].decode('utf-16-le'))
def apply(source,response,uri):
 edits=response.get('changes',{}).get(uri,[])
 for edit in sorted(edits,key=lambda e:offset(source,e['range']['start']),reverse=True):
  a=offset(source,edit['range']['start']);b=offset(source,edit['range']['end']);source=source[:a]+edit['newText']+source[b:]
 return source
results={}
for label,binary in [('baseline',root/'target/lsp-field-pattern/baseline-lsp'),('fixed',root/'target/p4/debug/dartforge-lsp'),('official',wrapper)]:
 with (out/(label+'.stderr')).open('w') as log:
  session=Session(binary,env,log);rows=[]
  try:
   session.request('initialize',{'processId':os.getpid(),'rootUri':out.as_uri(),'capabilities':{}});session.send('initialized',{})
   for name,source,new in cases:
    path=out/(name+'.dart');path.write_text(source);uri=path.as_uri()
    session.send('textDocument/didOpen',{'textDocument':{'uri':uri,'languageId':'dart','version':1,'text':source}})
    response=session.request('textDocument/rename',{'textDocument':{'uri':uri},'position':{'line':0,'character':source.splitlines()[0].index('value')+1},'newName':new})
    rows.append({'case':name,'source':source,'source_sha256':hashlib.sha256(source.encode()).hexdigest(),'rename':response,'applied':apply(source,response,uri)})
    session.send('textDocument/didClose',{'textDocument':{'uri':uri}})
  finally: session.close();(out/(label+'.transcript.json')).write_text(json.dumps(session.transcript,indent=2))
 results[label]={'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'rows':rows}
results['sdk_binary_sha256']=hashlib.sha256((sdk/'bin/dart').read_bytes()).hexdigest()
results['sdk_version']='3.6.2';results['sources']={p:hashlib.sha256((root/p).read_bytes()).hexdigest() for p in ['crates/lsp/src/project.rs','crates/lsp/src/rename.rs','crates/lsp/tests/rename_pattern_fields.rs']}
results['fixed_matches_official']=[a['applied']==b['applied'] for a,b in zip(results['fixed']['rows'],results['official']['rows'])]
results['baseline_matches_official']=[a['applied']==b['applied'] for a,b in zip(results['baseline']['rows'],results['official']['rows'])]
(out/'validation.json').write_text(json.dumps(results,indent=2))
print(json.dumps({k:results[k] for k in ['fixed_matches_official','baseline_matches_official']},indent=2))
assert all(results['fixed_matches_official'])

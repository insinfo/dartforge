import runpy,os,json,pathlib,hashlib
root=pathlib.Path.cwd();out=root/'target/lsp-pattern-selector/final';sdk=pathlib.Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk');Session=runpy.run_path(str(root/'scripts/verify-lsp-analyzer.py'))['Session'];sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
(out/'pubspec.yaml').write_text('name: selector_matrix\nenvironment:\n  sdk: ">=3.6.2 <4.0.0"\n');wrapper=out/'official-lsp';wrapper.write_text('#!/bin/sh\nexec '+str(sdk/'bin/dart')+' language-server --protocol=lsp\n');wrapper.chmod(0o755)
cases=[('getter','class C { int get value => 1; }\n',"import 'model.dart';\nint read(C c) { if (c case C(va|lue:final captured)) { return captured; } return 0; }\n"),('inherited','class Base { int value = 1; }\nclass C extends Base {}\nclass Outer { C get inner => C(); }\n',"import 'model.dart' as m;\nint read(m.Outer o) { if (o case m.Outer(inner:m.C(va|lue /* colon: keep */ :final captured))) { return captured; } return 0; }\n"),('extension','class C {}\nextension E on C { int get value => 1; }\n',"import 'model.dart';\nint read(C c) { if (c case C(va|lue:final captured)) { return captured; } return 0; }\n"),('shorthand','class C { int get value => 1; }\n',"import 'model.dart';\nint read(C c) { if (c case C(:final va|lue)) { return value; } return 0; }\n"),('record','',"int read(({int value}) r) { final (va|lue:final captured) = r; return captured; }\n"),('unresolved','class C {}\n',"import 'model.dart';\nint value = 7;\nint read(C c) { if (c case C(va|lue:final captured)) { return captured; } return 0; }\n")]
def position(source,offset):return {'line':source[:offset].count('\n'),'character':len(source[:offset].rsplit('\n',1)[-1].encode('utf-16-le'))//2}
def normalize(result):return (result[0] if result else None) if isinstance(result,list) else result
def apply(source,response,uri):
 edits=(response or {}).get('changes',{}).get(uri,[])
 def offset(p):
  lines=source.splitlines(keepends=True);return sum(map(len,lines[:p['line']]))+len(lines[p['line']].encode('utf-16-le')[:p['character']*2].decode('utf-16-le'))
 for edit in sorted(edits,key=lambda e:offset(e['range']['start']),reverse=True):
  a=offset(edit['range']['start']);b=offset(edit['range']['end']);source=source[:a]+edit['newText']+source[b:]
 return source
env=os.environ.copy();env['DART_SDK']=str(sdk);env['DARTFORGE_SDK_LIB']=str(sdk/'lib');result={}
for label,binary in [('baseline',root/'target/lsp-pattern-selector/baseline-lsp'),('fixed',root/'target/p4/debug/dartforge-lsp'),('official',wrapper)]:
 with (out/(label+'.stderr')).open('w') as log:
  session=Session(binary,env,log);rows=[]
  try:
   session.request('initialize',{'processId':os.getpid(),'rootUri':out.as_uri(),'capabilities':{}});session.send('initialized',{})
   for name,model,marked in cases:
    d=out/name;d.mkdir(exist_ok=True);mp=d/'model.dart';mp.write_text(model);source=marked.replace('|','');path=d/'main.dart';path.write_text(source);pos=position(source,marked.index('|'))
    session.send('textDocument/didOpen',{'textDocument':{'uri':path.as_uri(),'languageId':'dart','version':1,'text':source}})
    params={'textDocument':{'uri':path.as_uri()},'position':pos};definition=session.request('textDocument/definition',params)
    row={'case':name,'source':source,'model':model,'source_sha256':sha(path),'model_sha256':sha(mp),'cursor':pos,'definition':definition,'normalized_definition':normalize(definition)}
    if name=='getter':
     row['rename_from_selector']=session.request('textDocument/rename',{**params,'newName':'item'})
     if label=='official':
      session.send('textDocument/didOpen',{'textDocument':{'uri':mp.as_uri(),'languageId':'dart','version':1,'text':model}})
      row['rename_from_declaration']=session.request('textDocument/rename',{'textDocument':{'uri':mp.as_uri()},'position':position(model,model.index('value')+1),'newName':'item'})
      session.send('textDocument/didClose',{'textDocument':{'uri':mp.as_uri()}})
     edit=row.get('rename_from_declaration') if label=='official' else row['rename_from_selector']
     row['applied_rename_model']=apply(model,edit,mp.as_uri());row['applied_rename_source']=apply(source,edit,path.as_uri())
    rows.append(row);session.send('textDocument/didClose',{'textDocument':{'uri':path.as_uri()}})
  finally:session.close();(out/(label+'.transcript.json')).write_text(json.dumps(session.transcript,indent=2))
 result[label]={'binary_sha256':sha(binary),'rows':rows}
result['sdk_version']='3.6.2';result['sdk_binary_sha256']=sha(sdk/'bin/dart');result['source_sha256']={p:sha(root/p) for p in ['crates/lsp/src/project.rs','crates/lsp/tests/pattern_selector_navigation.rs']}
result['definitions_fixed_exact_sdk']=[a['normalized_definition']==b['normalized_definition'] for a,b in zip(result['fixed']['rows'],result['official']['rows'])];result['definitions_baseline_exact_sdk']=[a['normalized_definition']==b['normalized_definition'] for a,b in zip(result['baseline']['rows'],result['official']['rows'])]
a=result['fixed']['rows'][0];b=result['official']['rows'][0];result['forge_selector_rename_matches_sdk_declaration_rename']=all(a[k]==b[k] for k in ['applied_rename_source','applied_rename_model']);result['rename_scope_note']='SDK3.6.2 direct rename from pattern selector returns null. Forge now supports it through resolved member identity; its applied edit is compared with SDK rename from the member declaration, not asserted equal for the same cursor.'
(out/'validation.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:result[k] for k in ['definitions_fixed_exact_sdk','definitions_baseline_exact_sdk','forge_selector_rename_matches_sdk_declaration_rename']},indent=2));assert all(result['definitions_fixed_exact_sdk']);assert result['forge_selector_rename_matches_sdk_declaration_rename']

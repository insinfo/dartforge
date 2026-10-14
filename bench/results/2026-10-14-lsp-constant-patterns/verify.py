import runpy,os,json,pathlib,hashlib
root=pathlib.Path.cwd();out=root/'target/lsp-constant-pattern/final';sdk=pathlib.Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk');Session=runpy.run_path(str(root/'scripts/verify-lsp-analyzer.py'))['Session'];sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
(out/'pubspec.yaml').write_text('name: constant_pattern_matrix\nenvironment:\n  sdk: ">=3.6.2 <4.0.0"\n');wrapper=out/'official-lsp';wrapper.write_text('#!/bin/sh\nexec '+str(sdk/'bin/dart')+' language-server --protocol=lsp\n');wrapper.chmod(0o755)
cases=[
 ('top','',"const threshold = 1;\nint read(int x) => switch (x) { th|reshold => 42, _ => 0 };\n",'threshold'),
 ('imported',"const threshold = 1;\n", "import 'model.dart' show threshold;\nimport 'model.dart' as a;\nimport 'other.dart' as b;\nint read(int x) => switch (x) { th|reshold => a.threshold + b.threshold, _ => 0 };\nint binding(int x) { if (x case final threshold) { return threshold; } return 0; }\n",'threshold'),
 ('static','',"class C { static const threshold = 1; static int read(int x) => switch (x) { th|reshold => 42, _ => 0 }; }\n",'threshold'),
 ('enum','',"enum E { first, second; int read() => switch (this) { fi|rst => 1, _ => 0 }; }\n",'first'),
 ('list','',"const threshold = 1;\nint read(List<int> values) { if (values case [th|reshold, threshold]) { return 1; } return 0; }\n",'threshold'),
 ('binding','',"const threshold = 1;\nint read(int x) { if (x case final th|reshold) { return threshold; } return 0; }\n",None),
 ('unresolved','',"int read(int x) => switch (x) { mi|ssing => 1, _ => 0 };\nint other() { const missing = 1; return missing; }\n",None),
]
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
for label,binary in [('baseline',root/'target/lsp-constant-pattern/baseline-lsp'),('fixed',root/'target/p4/debug/dartforge-lsp'),('official',wrapper)]:
 with (out/(label+'.stderr')).open('w') as log:
  session=Session(binary,env,log);rows=[]
  try:
   session.request('initialize',{'processId':os.getpid(),'rootUri':out.as_uri(),'capabilities':{}});session.send('initialized',{})
   for name,model,marked,constant in cases:
    d=out/name;d.mkdir(exist_ok=True);mp=d/'model.dart';mp.write_text(model);(d/'other.dart').write_text('const threshold = 2;\n');source=marked.replace('|','');path=d/'main.dart';path.write_text(source);pos=position(source,marked.index('|'))
    session.send('textDocument/didOpen',{'textDocument':{'uri':path.as_uri(),'languageId':'dart','version':1,'text':source}})
    params={'textDocument':{'uri':path.as_uri()},'position':pos};definition=session.request('textDocument/definition',params)
    row={'case':name,'source':source,'model':model,'source_sha256':sha(path),'model_sha256':sha(mp),'cursor':pos,'definition':definition,'normalized_definition':normalize(definition)}
    if constant:
     row['rename_from_pattern']=session.request('textDocument/rename',{**params,'newName':'item'})
     row['applied_rename_source']=apply(source,row['rename_from_pattern'],path.as_uri())
     row['applied_rename_model']=apply(model,row['rename_from_pattern'],mp.as_uri())
     declaration_path=mp if model else path
     declaration_source=model if model else source
     if model:session.send('textDocument/didOpen',{'textDocument':{'uri':mp.as_uri(),'languageId':'dart','version':1,'text':model}})
     declaration_params={'textDocument':{'uri':declaration_path.as_uri()},'position':position(declaration_source,declaration_source.index(constant)+1)}
     row['rename_from_declaration']=session.request('textDocument/rename',{**declaration_params,'newName':'item'})
     row['declaration_applied_source']=apply(source,row['rename_from_declaration'],path.as_uri())
     row['declaration_applied_model']=apply(model,row['rename_from_declaration'],mp.as_uri())
     row['references_from_declaration']=session.request('textDocument/references',{**declaration_params,'context':{'includeDeclaration':True}})
     if model:session.send('textDocument/didClose',{'textDocument':{'uri':mp.as_uri()}})
    rows.append(row);session.send('textDocument/didClose',{'textDocument':{'uri':path.as_uri()}})
  finally:session.close();(out/(label+'.transcript.json')).write_text(json.dumps(session.transcript,indent=2))
 result[label]={'binary_sha256':sha(binary),'rows':rows}
result['sdk_version']='3.6.2';result['sdk_binary_sha256']=sha(sdk/'bin/dart');result['source_sha256']={p:sha(root/p) for p in ['crates/lsp/src/project.rs','crates/lsp/tests/constant_pattern_navigation.rs']}
result['definitions_fixed_exact_sdk']=[a['normalized_definition']==b['normalized_definition'] for a,b in zip(result['fixed']['rows'],result['official']['rows'])];result['definitions_baseline_exact_sdk']=[a['normalized_definition']==b['normalized_definition'] for a,b in zip(result['baseline']['rows'],result['official']['rows'])]
def sorted_locations(locations):return sorted(locations or [],key=lambda p:(p['uri'],p['range']['start']['line'],p['range']['start']['character']))
result['applied_rename_fixed_exact_sdk']=[all(a[k]==b[k] for k in ['applied_rename_source','applied_rename_model','declaration_applied_source','declaration_applied_model']) for a,b in zip(result['fixed']['rows'],result['official']['rows']) if 'rename_from_pattern' in a]
result['references_fixed_exact_sdk']=[sorted_locations(a['references_from_declaration'])==sorted_locations(b['references_from_declaration']) for a,b in zip(result['fixed']['rows'],result['official']['rows']) if 'references_from_declaration' in a]
(out/'validation.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:result[k] for k in ['definitions_fixed_exact_sdk','definitions_baseline_exact_sdk','applied_rename_fixed_exact_sdk','references_fixed_exact_sdk']},indent=2));assert all(result['definitions_fixed_exact_sdk']);assert all(result['applied_rename_fixed_exact_sdk']);assert all(result['references_fixed_exact_sdk'])

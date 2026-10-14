from pathlib import Path
import os,subprocess,json,hashlib,threading,time,runpy,argparse
parser=argparse.ArgumentParser();parser.add_argument("--quiet-window-authorized",action="store_true");parser.add_argument("--round",type=int,required=True);args=parser.parse_args();assert args.quiet_window_authorized,"Timing requires root quiet-window grant"
root=Path.cwd();out=root/('bench/results/2026-10-14-current-tools-status/lsp-latency/round-'+str(args.round));out.mkdir(parents=True,exist_ok=True)
work=root/'target/current-tools-status-lsp';(work/'lib').mkdir(parents=True,exist_ok=True)
sdk=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk');native=root/'target/current-toolchain-status/dartforge-lsp'
Session=runpy.run_path(str(root/'scripts/verify-lsp-analyzer.py'))['Session']
request_timings=[]
original_request=Session.request
def timed_request(self,method,params):
 start=time.perf_counter();result=original_request(self,method,params);elapsed=time.perf_counter()-start
 if method.startswith('textDocument/'):
  request_timings.append({'server':label,'method':method,'params':params,'client_seconds':elapsed})
 return result
Session.request=timed_request
wrapper=work/'official-lsp';wrapper.write_text('#!/bin/sh\nexec '+str(sdk/'bin/dart')+' language-server --protocol=lsp\n');wrapper.chmod(0o755)
(work/'pubspec.yaml').write_text('name: current_tools_lsp\nenvironment:\n  sdk: ^3.6.0\n');(work/'.dart_tool').mkdir(exist_ok=True)
(work/'.dart_tool/package_config.json').write_text(json.dumps({'configVersion':2,'packages':[{'name':'current_tools_lsp','rootUri':'../','packageUri':'lib/','languageVersion':'3.6'}]}))
sources={
'helper.dart':"class Person {\n  final String name;\n  Person(this.name);\n  String greet() => 'Hello $name';\n}\nString greeting(Person person) => person.greet();\n",
'main.dart':"import 'helper.dart';\nimport 'dart:typed_data';\nvoid main() {\n  final person = Person('Ada');\n  final message = greeting(person);\n  print(message.trim());\n  print(Uint8List.fromList([65]));\n}\n",
'patterns.dart':"class Box { final int value; Box(this.value); }\nint read(Box box) { if (box case Box(:final value)) { return value; } return 0; }\n",
'format.dart':"void main(){final values=<int>[1,2,3];for(final value in values){print(value);}}\n",
}
for name,text in sources.items():(work/'lib'/name).write_text(text)
queries=[('cross_file_function','main.dart','greeting(person)',1,'welcome'),('cross_file_class','main.dart',"Person('Ada')",1,'Individual'),('sdk_class','main.dart','Uint8List.fromList',1,None),('sdk_method','main.dart','trim()',1,None),('pattern_binding','patterns.dart','return value',8,'item'),('pattern_field','patterns.dart','int value',5,'item')]
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def pos(text,offset):
 before=text[:offset];return {'line':before.count('\n'),'character':len(before.rsplit('\n',1)[-1].encode('utf-16-le'))//2}
def offset(text,p):
 lines=text.splitlines(keepends=True);return sum(len(x) for x in lines[:p['line']])+len(lines[p['line']].encode('utf-16-le')[:p['character']*2].decode('utf-16-le'))
def locations(reply):
 if reply is None:return []
 if not isinstance(reply,list):reply=[reply]
 return sorted((x.get('targetUri',x.get('uri')),json.dumps(x.get('targetSelectionRange',x.get('range')),sort_keys=True)) for x in reply)
def changes(reply):
 result=dict(reply.get('changes',{}))
 for doc in reply.get('documentChanges',[]):
  if 'textDocument' in doc:result.setdefault(doc['textDocument']['uri'],[]).extend(doc['edits'])
 return result
def applied(reply):
 result={}
 for uri,edits in changes(reply).items():
  name=Path(uri).name;text=sources[name]
  for edit in sorted(edits,key=lambda e:offset(text,e['range']['start']),reverse=True):
   a,b=offset(text,edit['range']['start']),offset(text,edit['range']['end']);text=text[:a]+edit['newText']+text[b:]
  result[uri]=text
 return result
class Rss:
 def __init__(self,pid):pass
 def finish(self):return {'not_measured_in_latency_window':True}
ledger={'compiler':str(native),'compiler_sha256':sha(native),'sdk_binary_sha256':sha(sdk/'bin/dart'),'sdk_version':'3.6.2','timing_measured':True,'quiet_window_authorized':True,'sampling':'one client roundtrip per original scoped query; initialize/diagnostic wait/shutdown excluded; no RSS sampler','fixture_sha256':{name:hashlib.sha256(text.encode()).hexdigest() for name,text in sources.items()},'servers':{},'comparisons':[],'complete':False}
order=[('native',native),('sdk',wrapper)]
if args.round%2:order.reverse()
for label,binary in order:
 env=dict(os.environ,DARTFORGE_SDK_LIB=str(sdk/'lib'),DART_SDK=str(sdk),DARTFORGE_LSP_MEMORY='0',TMPDIR=str(root/'target/tmp-p8-runtime'));env.pop('DARTFORGE_LSP_TYPED_IN_PROCESS',None);env.pop('RUST_LOG',None);env.pop('DARTFORGE_LSP_DEBUG',None)
 with (out/(label+'.stderr')).open('w') as log:
  session=Session(binary,env,log);rss=Rss(session.process.pid);results={}
  try:
   results['initialize']=session.request('initialize',{'processId':os.getpid(),'rootUri':work.as_uri(),'workspaceFolders':[{'uri':work.as_uri(),'name':work.name}],'capabilities':{'textDocument':{'rename':{'prepareSupport':True},'publishDiagnostics':{'versionSupport':True}}}});session.send('initialized',{})
   for name,text in sources.items():session.send('textDocument/didOpen',{'textDocument':{'uri':(work/'lib'/name).as_uri(),'languageId':'dart','version':1,'text':text}})
   for case,name,token,delta,new_name in queries:
    uri=(work/'lib'/name).as_uri();params={'textDocument':{'uri':uri},'position':pos(sources[name],sources[name].index(token)+delta)}
    for method in ['definition','references']+(['rename'] if new_name else []):
     payload=dict(params)
     if method=='references':payload['context']={'includeDeclaration':True}
     if method=='rename':payload['newName']=new_name
     try:reply=session.request('textDocument/'+method,payload);results[case+'/'+method]={'reply':reply,'normalized':applied(reply) if method=='rename' else locations(reply)}
     except Exception as error:results[case+'/'+method]={'error':str(error)}
   uri=(work/'lib/format.dart').as_uri();reply=session.request('textDocument/formatting',{'textDocument':{'uri':uri},'options':{'tabSize':2,'insertSpaces':True}});results['format/formatting']={'reply':reply,'normalized':applied({'changes':{uri:reply or []}})}
   # Wait for a typed diagnostic, proving the default isolated typed-analysis path.
   bad_uri=(work/'lib/main.dart').as_uri();session.send('textDocument/didChange',{'textDocument':{'uri':bad_uri,'version':2},'contentChanges':[{'text':'void main() { if (1) {} }\n'}]})
   diagnostic=session.wait(lambda m:m.get('method')=='textDocument/publishDiagnostics' and m['params'].get('uri')==bad_uri and any(d.get('code')=='non_bool_condition' for d in m['params']['diagnostics']))
   results['typed_diagnostics']=diagnostic
  finally:
   session.close();ledger['servers'][label]={'command':[str(binary),'--stdio'],'results':results,'memory':rss.finish()};(out/(label+'.transcript.json')).write_text(json.dumps(session.transcript,indent=2)+'\n')
for key,value in ledger['servers']['native']['results'].items():
 if key in ['initialize','typed_diagnostics']:continue
 official=ledger['servers']['sdk']['results'].get(key,{})
 ledger['comparisons'].append({'case_method':key,'exact_normalized':value.get('normalized')==official.get('normalized') and 'error' not in value and 'error' not in official,'native':value,'sdk':official})
ledger['request_timings']=request_timings
ledger['passed']=sum(x['exact_normalized'] for x in ledger['comparisons']);ledger['compared']=len(ledger['comparisons']);ledger['complete']=True;ledger['compiler_unchanged']=sha(native)==ledger['compiler_sha256']
(out/'validation.json').write_text(json.dumps(ledger,indent=2)+'\n');print('LSP normalized exact',ledger['passed'],'/',ledger['compared'],flush=True)
assert ledger['compiler_unchanged']

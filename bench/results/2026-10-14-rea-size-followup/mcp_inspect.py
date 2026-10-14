import subprocess,os,json,pathlib,sys,time,hashlib
base=pathlib.Path(__file__).resolve().parent;cfg=json.load(open(sys.argv[1]));out=base/cfg['label'];out.mkdir(exist_ok=False)
p=subprocess.Popen(['/Volumes/ssd_dock/tools/rea/node-v24.21.0-darwin-arm64/bin/node','/Volumes/ssd_dock/tools/rea/node_modules/rea-agents/scripts/rea.mjs','mcp'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=open(out/'server.stderr','w'),text=True)
nextid=0

def request(method,params,label):
 global nextid
 nextid+=1;i=nextid;message={'jsonrpc':'2.0','id':i,'method':method,'params':params};(out/(label+'-request.json')).write_text(json.dumps(message,indent=2));start=time.time();p.stdin.write(json.dumps(message)+'\n');p.stdin.flush()
 while True:
  line=p.stdout.readline()
  if not line:raise RuntimeError('REA MCP stopped')
  r=json.loads(line)
  if r.get('id')==i:
   (out/(label+'.json')).write_text(json.dumps(r,indent=2));print(label,'reply',round(time.time()-start,1),flush=True);return r
  with (out/'notifications.jsonl').open('a') as f:f.write(line)

def tool(name,args,label):return request('tools/call',{'name':name,'arguments':args},label)
try:
 request('initialize',{'protocolVersion':'2024-11-05','capabilities':{},'clientInfo':{'name':'local-native-audit','version':'1.0'}},'initialize');p.stdin.write(json.dumps({'jsonrpc':'2.0','method':'notifications/initialized'})+'\n');p.stdin.flush()
 tool('open_binary',{'path':cfg['path'],'provider_id':'ghidra',**cfg.get('open_options',{})},'open')
 tool('binary_overview',{},'overview')
 for item in cfg['procedures']:
  for operation in ['procedure_assembly','procedure_pseudo_code','procedure_callees','procedure_callers']:
   tool(operation,{'procedure':item['address']},item['label']+'-'+operation)
 tool('close_binary',{'snapshot_path':str(out/'snapshot.json')},'close')
finally:
 p.stdin.close()
 try:p.wait(timeout=20)
 except subprocess.TimeoutExpired:p.terminate();p.wait(timeout=20)
 (out/'manifest.json').write_text(json.dumps({'config':cfg,'target_sha256':hashlib.sha256(pathlib.Path(cfg['path']).read_bytes()).hexdigest(),'REA_version':'6.5.0','provider':'Ghidra12.1.4/JDK21.0.12.1/nativeARM64','server_exit':p.returncode,'scope':'Local static native assembly and heuristic C pseudocode; Dart custom snapshot ABI is not interpreted as recovered Dart semantics.'},indent=2))

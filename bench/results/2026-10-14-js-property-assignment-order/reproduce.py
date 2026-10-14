import pathlib,subprocess,hashlib,json
root=pathlib.Path.cwd(); base=root/'target/tmp-js-production-audit/property-assignment'; out=root/'bench/results/2026-10-14-js-property-assignment-order';out.mkdir(parents=True,exist_ok=True)
source=root/'crates/emit_js/tests/programs/property_assignment_order.dart';dart='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart';node='/Applications/ChatGPT.app/Contents/Resources/cua_node/bin/node'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
ledger={'scope':'Correctness only; no size or performance claim. Same final fixture for before/after.','source_sha256':sha(source),'runs':{},'compiler_sha256':{}}
def run(tag,args):
 r=subprocess.run(args,capture_output=True);(out/(tag+'.stdout')).write_bytes(r.stdout);(out/(tag+'.stderr')).write_bytes(r.stderr);return {'command':list(map(str,args)),'exit':r.returncode,'stdout_sha256':hashlib.sha256(r.stdout).hexdigest()}
ledger['runs']['VM']=run('VM',[dart,'run',str(source)])
for label,compiler in [('baseline',base/'baseline-jsprod'),('current',root/'target/js/debug/dartforge-jsprod')]:
 ledger['compiler_sha256'][label]=sha(compiler)
 for profile in ['raw','min']:
  tag=label+'.'+profile; artifact=base/(tag+'.js'); args=[str(compiler)]+(['--no-minify'] if profile=='raw' else [])+[str(source),'-o',str(artifact)]
  ledger['runs'][tag+'.compile']=run(tag+'.compile',args)
  ledger['runs'][tag]=run(tag,[node,'-e','globalThis.self=globalThis;require(process.argv[1])',str(artifact)])
  ledger['runs'][tag]['artifact_sha256']=sha(artifact)
artifact=base/'final.O2.js';ledger['runs']['O2.compile']=run('O2.compile',[dart,'compile','js','-O2',str(source),'-o',str(artifact)]);ledger['runs']['O2']=run('O2',[node,'-e','globalThis.self=globalThis;require(process.argv[1])',str(artifact)]);ledger['runs']['O2']['artifact_sha256']=sha(artifact)
ledger['source_files_sha256']={p:sha(root/p) for p in ['crates/emit_js/src/expr.rs','crates/emit_js/tests/basic.rs','crates/emit_js/tests/programs/property_assignment_order.dart','crates/emit_js_production/tests/property_assignment_order.rs']}
(out/'validation.json').write_text(json.dumps(ledger,indent=2)+'\n');print(json.dumps({k:v['exit'] for k,v in ledger['runs'].items()}))

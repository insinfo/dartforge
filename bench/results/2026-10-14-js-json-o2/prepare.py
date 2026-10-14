from pathlib import Path
import subprocess,json,hashlib,os,gzip
root=Path.cwd();out=root/'target/tmp-js-production-audit/json-collections';compiler=root/'target/tmp-js-production-audit/const-cache-use/final-jsprod';sdk=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk');sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert sha(compiler)=='96a34ad807a15943ba5ab041653e7d64b84ff76e99a7331cf32278592eedfa7a';inputs={str(root/'bench/performance'/n):sha(root/'bench/performance'/n) for n in ['json.dart','collections.dart','common.dart']};ledger={'compiler':str(compiler),'compiler_sha256':sha(compiler),'sdk':str(sdk),'inputs':inputs,'artifacts':{}};env=dict(os.environ,TMPDIR=str(out))
correct=out/'correctness';correct.mkdir(exist_ok=True)
for case in ['json','collections']:(correct/f'{case}.dart').write_bytes((root/f'bench/performance/{case}.dart').read_bytes())
(correct/'common.dart').write_text("void measure(String name, Object? Function() kernel, {int rounds = 6}) { print('$name: ${kernel()}'); }\n")
for scope in ['timing','correctness']:
 for case in ['json','collections']:
  source=root/f'bench/performance/{case}.dart' if scope=='timing' else correct/f'{case}.dart';profiles={}
  for profile in ['raw','min','O2']:
   folder=out/scope/case/profile;folder.mkdir(parents=True,exist_ok=True);artifact=folder/'program.js'
   cmd=([str(sdk/'bin/dart'),'compile','js','-O2',str(source),'-o',str(artifact)] if profile=='O2' else [str(compiler),str(source),'--sdk',str(sdk/'lib'),'-o',str(artifact)]+(['--no-minify'] if profile=='raw' else []));r=subprocess.run(cmd,capture_output=True,text=True,env=env,timeout=180);(folder/'compile.stdout').write_text(r.stdout);(folder/'compile.stderr').write_text(r.stderr);profiles[profile]={'command':cmd,'exit':r.returncode};print(scope,case,profile,r.returncode,flush=True)
   if r.returncode==0:
    raw=artifact.read_bytes();profiles[profile].update({'artifact':str(artifact),'sha256':sha(artifact),'bytes':len(raw),'gzip9_bytes':len(gzip.compress(raw,compresslevel=9,mtime=0))})
  ledger['artifacts'][scope+'.'+case]=profiles
assert all(sha(p)==h for p,h in inputs.items());ledger['inputs_stable']=True;ledger['scope']='Artifact preparation only, no performance timings. Correctness snapshot uses byte-identical benchmark bodies with a one-call untimed common.dart harness; timing artifacts use original common.dart.';ledger['correctness_harness_sha256']=sha(correct/'common.dart');(out/'artifacts.json').write_text(json.dumps(ledger,indent=2)+'\n')

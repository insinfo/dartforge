from pathlib import Path
import subprocess,os,json,hashlib,re
root=Path.cwd();out=root/'target/checked-double-roundtrip/inherited-observer';producer=root/'target/numeric-scalar-producer/dartforge';sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();assert sha(producer)=='1d2dadbc030f9da15f4672900ad1f9aab1ea736edb0acff6be82174b8d620655';entry=out/'main.dart';patch=out/'sdk/lib/_internal/vm/lib/object_patch.dart';inputs={str(p):sha(p) for p in [producer,entry,patch]};ledger={'inputs':inputs,'commands':[],'cases':[],'scope':'Selected modified SDK inheritance observer witness; Forgebefore/after only, no upstream modifiedSDKexecutionclaim.'}
def save():(out/'validation.json').write_text(json.dumps(ledger,indent=2)+'\n')
base=dict(os.environ,TMPDIR=str(out),DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang',DARTFORGE_KEEP_IR='1')
for k in ['DARTFORGE_MEMORY','DARTFORGE_ARC_PREPARED','DARTFORGE_GC_STRESS','DARTFORGE_NO_CHECKED_DOUBLE_ROUNDTRIP','DARTFORGE_NO_SCALAR_COALESCE']:base.pop(k,None)
for enabled in [False,True]:
 label='on' if enabled else 'off';case=out/label;case.mkdir(exist_ok=False);env=base.copy()
 if not enabled:env['DARTFORGE_NO_CHECKED_DOUBLE_ROUNDTRIP']='1'
 command=[str(producer),'compile-native',str(entry),'--sdk',str(out/'sdk/lib'),'--optimize','--memory=tracing','--exceptions=check','--roots=shadow','--stack-trace=none','-o',str(case/'program')]
 with (case/'compile.stdout').open('wb') as so,(case/'compile.stderr').open('wb') as se:r=subprocess.run(command,env=env,stdout=so,stderr=se,timeout=1200)
 row={'enabled':enabled,'compile_exit':r.returncode,'command':command};ledger['cases'].append(row);save();print(label+'-compile',r.returncode,flush=True)
 if r.returncode:raise SystemExit(r.returncode)
 with (case/'run.stdout').open('wb') as so,(case/'run.stderr').open('wb') as se:r=subprocess.run([str(case/'program')],env=env,stdout=so,stderr=se,timeout=60)
 row.update(run_exit=r.returncode,stdout=(case/'run.stdout').read_text(),stderr=(case/'run.stderr').read_text(),artifact_sha256=sha(case/'program'))
 text=(case/'.df_tmp/program.ll').read_text();body=re.findall(r'define[^\n]*@[^\n]*\.roundTrip\([^\n]*\).*?\n\}',text,re.S);assert len(body)==1;(case/'roundTrip.ll').write_text(body[0]+'\n');row['double_boxes']=len(re.findall(r'call[^\n]*@df.box_double\(',body[0]));save();print(label,row['run_exit'],row['stdout'].strip(),'BOX',row['double_boxes'],flush=True)
ledger.update(inputs_unchanged=all(sha(Path(p))==h for p,h in inputs.items()),result='OBSERVED_DIVERGENCE' if ledger['cases'][0]['stdout']!=ledger['cases'][1]['stdout'] else 'SAME_OUTPUT');save();print(ledger['result'],flush=True)

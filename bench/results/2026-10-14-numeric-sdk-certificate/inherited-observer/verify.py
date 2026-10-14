from pathlib import Path
import subprocess,os,json,hashlib,re,argparse
parser=argparse.ArgumentParser()
parser.add_argument('--producer',required=True,type=Path)
parser.add_argument('--producer-sha256',required=True)
parser.add_argument('--output',required=True,type=Path)
args=parser.parse_args()
root=Path.cwd();out=args.output.resolve();out.mkdir(parents=True,exist_ok=False);producer=args.producer.resolve();sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();assert sha(producer)==args.producer_sha256
selected=root/'target/checked-double-roundtrip/inherited-observer';entry=selected/'main.dart';patch=selected/'sdk/lib/_internal/vm/lib/object_patch.dart';inputs={str(p):sha(p) for p in [producer,entry,patch]};ledger={'inputs':inputs,'commands':[],'cases':[],'scope':'Previously divergent selected modified SDK inheritance observer. Both modes must preserve the original boxed identity distinction; no upstream modified-SDK execution claim.'}
def save():(out/'validation.json').write_text(json.dumps(ledger,indent=2)+'\n')
base=dict(os.environ,TMPDIR=str(out),DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang',DARTFORGE_KEEP_IR='1')
for k in ['DARTFORGE_MEMORY','DARTFORGE_ARC_PREPARED','DARTFORGE_GC_STRESS','DARTFORGE_NO_CHECKED_DOUBLE_ROUNDTRIP','DARTFORGE_NO_SCALAR_COALESCE']:base.pop(k,None)
for enabled in [False,True]:
 label='on' if enabled else 'off';case=out/label;case.mkdir(exist_ok=False);env=base.copy()
 if not enabled:env['DARTFORGE_NO_CHECKED_DOUBLE_ROUNDTRIP']='1'
 command=[str(producer),'compile-native',str(entry),'--sdk',str(selected/'sdk/lib'),'--optimize','--memory=tracing','--exceptions=check','--roots=shadow','--stack-trace=none','-o',str(case/'program')]
 with (case/'compile.stdout').open('wb') as so,(case/'compile.stderr').open('wb') as se:r=subprocess.run(command,env=env,stdout=so,stderr=se,timeout=1200)
 row={'enabled':enabled,'compile_exit':r.returncode,'command':command};ledger['cases'].append(row);save();print(label+'-compile',r.returncode,flush=True)
 if r.returncode:raise SystemExit(r.returncode)
 with (case/'run.stdout').open('wb') as so,(case/'run.stderr').open('wb') as se:r=subprocess.run([str(case/'program')],env=env,stdout=so,stderr=se,timeout=60)
 row.update(run_exit=r.returncode,stdout=(case/'run.stdout').read_text(),stderr=(case/'run.stderr').read_text(),artifact_sha256=sha(case/'program'))
 text=(case/'.df_tmp/program.ll').read_text();body=re.findall(r'define[^\n]*@[^\n]*\.roundTrip\([^\n]*\).*?\n\}',text,re.S);assert len(body)==1;(case/'roundTrip.ll').write_text(body[0]+'\n');row['double_boxes']=len(re.findall(r'call[^\n]*@df.box_double\(',body[0]));save();print(label,row['run_exit'],row['stdout'].strip(),'BOX',row['double_boxes'],flush=True)
ledger.update(inputs_unchanged=all(sha(Path(p))==h for p,h in inputs.items()))
ledger['passed']=ledger['inputs_unchanged'] and all(c['run_exit']==0 and c['stdout']=='false\n' and c['stderr']=='' and c['double_boxes']==1 for c in ledger['cases'])
save();print('PASS='+str(ledger['passed']),flush=True)
assert ledger['passed'], 'The real modified-SDK counterexample must preserve both boxes and output false in both modes.'

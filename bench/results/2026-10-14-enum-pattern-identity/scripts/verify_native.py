from pathlib import Path
import subprocess,os,json,hashlib,time,shutil,sys
root=Path.cwd();out=root/'target/enum-pattern-identity/native-final';out.mkdir(exist_ok=False);producer=root/'target/native-receiver-producer/dartforge';sdk=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk');clang=Path('/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang');fixture=out/'fixture';shutil.copytree(root/'crates/emit_native/tests/fixtures/enum_pattern_identity',fixture);entry=fixture/'main.dart';sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();sources={str(p):sha(p) for p in fixture.rglob('*.dart')};producerhash=sha(producer);assert producerhash=='bee770c7623e94439a5df1d5b792af9c53577c682398c6491c379befe51ef171';expected=(root/'target/enum-pattern-identity/oracle/vm-final.stdout').read_bytes();ledger={'producer':str(producer),'producer_sha256':producerhash,'source_before':sources,'expected_stdout':expected.decode(),'cases':[],'scope':'Actual immutable native AOT execution, same source ON/OFF, normal/GCstress in tracing/legacyARC/preparedARC. Correctness only; no timing/performance claim.'}
def save(): (out/'validation.json').write_text(json.dumps(ledger,indent=2)+'\n')
for mode in ['tracing','arc','prepared']:
 for enabled in [False,True]:
  case=out/(mode+('-on' if enabled else '-off'));case.mkdir();tmp=case/'tmp';tmp.mkdir();env=dict(os.environ,TMPDIR=str(tmp),DARTFORGE_CLANG=str(clang),LLVM_SYS_221_PREFIX=str(clang.parents[1]),DARTFORGE_SDK_LIB=str(sdk/'lib'))
  for k in ['DARTFORGE_MEMORY','DARTFORGE_GC_STRESS','DARTFORGE_ARC_PREPARED','DARTFORGE_ARC_PREPARE','DARTFORGE_ARC_CHECK','DARTFORGE_EFFECTS','DARTFORGE_NO_ENUM_PATTERN_IDENTITY']:env.pop(k,None)
  if not enabled:env['DARTFORGE_NO_ENUM_PATTERN_IDENTITY']='1'
  if mode=='prepared':env['DARTFORGE_ARC_PREPARED']='1'
  if mode!='tracing':env.update(DARTFORGE_ARC_CHECK='1',DARTFORGE_EFFECTS='check')
  cmd=[str(producer),'compile-native',str(entry),'--sdk',str(sdk/'lib'),'--optimize','--memory='+('tracing' if mode=='tracing' else 'arc'),'--exceptions=check','--roots=shadow','--stack-trace=none','-o',str(case/'program')];row={'mode':mode,'enabled':enabled,'compile_command':cmd,'environment':{k:env.get(k) for k in ['TMPDIR','DARTFORGE_ARC_PREPARED','DARTFORGE_NO_ENUM_PATTERN_IDENTITY','DARTFORGE_ARC_CHECK','DARTFORGE_EFFECTS']}}
  with (case/'compile.stdout').open('wb') as stdout,(case/'compile.stderr').open('wb') as stderr:r=subprocess.run(cmd,env=env,stdout=stdout,stderr=stderr,timeout=1200)
  row['compile_exit']=r.returncode;ledger['cases'].append(row);save();print(case.name,'compile',r.returncode,flush=True)
  if r.returncode:raise SystemExit('Actual native compilation failed: '+case.name)
  row['artifact_sha256']=sha(case/'program');row['manifest_sha256']=sha(case/'program.contracts.json');row['runs']=[]
  for stress in [False,True]:
   runenv=env.copy()
   if stress:runenv['DARTFORGE_GC_STRESS']='1'
   label='stress' if stress else 'normal';r=subprocess.run([str(case/'program')],env=runenv,capture_output=True,timeout=40);(case/(label+'.stdout')).write_bytes(r.stdout);(case/(label+'.stderr')).write_bytes(r.stderr);ok=r.returncode==0 and r.stdout==expected and not r.stderr;row['runs'].append({'stress':stress,'exit':r.returncode,'exact_oracle':r.stdout==expected,'stderr_bytes':len(r.stderr),'pass':ok});save();print(case.name,label,'PASS' if ok else 'FAIL',flush=True)
   if not ok:raise SystemExit('Native semantic/exit failure: '+case.name+'/'+label)
ledger.update({'producer_unchanged':sha(producer)==producerhash,'sources_unchanged':all(sha(Path(p))==s for p,s in sources.items()),'result':'PASS','native_executions':sum(len(c['runs']) for c in ledger['cases'])});save();assert ledger['producer_unchanged'] and ledger['sources_unchanged'];print('ALL_NATIVE_PASS',ledger['native_executions'],flush=True)

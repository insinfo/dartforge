from pathlib import Path
import os,json,subprocess,hashlib,re,shutil
root=Path.cwd();out=root/'target/enum-pattern-identity/format-final';out.mkdir(exist_ok=False);sdk=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk');producer=root/'target/native-receiver-producer/dartforge';package=Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui/examples/file_explorer/.dart_tool/package_config.json');actual=Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui/lib/src/graphics/image/codecs/color/format.dart');entry=out/'probe.dart';shutil.copyfile(root/'target/enum-pattern-identity/format_probe.dart',entry);sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();inputs={str(p):sha(p) for p in [producer,package,actual,entry]};assert inputs[str(producer)]=='bee770c7623e94439a5df1d5b792af9c53577c682398c6491c379befe51ef171';ledger={'inputs_before':inputs,'commands':[],'cases':[],'scope':'Actual imported convertFormatValue, same compiler enum ON/OFF. 12x12 formats x15 values including casts,NaN,Infinity,signedzero andfinitefloatbits. No timing/performance claim.'}
def save():(out/'validation.json').write_text(json.dumps(ledger,indent=2)+'\n')
def run(label,cmd,env,timeout=1200):
 with (out/(label+'.stdout')).open('wb') as stdout,(out/(label+'.stderr')).open('wb') as stderr:r=subprocess.run(cmd,env=env,stdout=stdout,stderr=stderr,timeout=timeout)
 ledger['commands'].append({'label':label,'command':cmd,'exit':r.returncode,'environment':{k:env.get(k) for k in ['TMPDIR','DARTFORGE_NO_ENUM_PATTERN_IDENTITY','DARTFORGE_KEEP_IR','DARTFORGE_LINK_MAP']}});save();print(label,r.returncode,flush=True);return r.returncode
base=dict(os.environ,TMPDIR=str(out),DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang',DARTFORGE_SDK_LIB=str(sdk/'lib'))
for k in ['DARTFORGE_MEMORY','DARTFORGE_GC_STRESS','DARTFORGE_ARC_PREPARED','DARTFORGE_ARC_PREPARE','DARTFORGE_ARC_CHECK','DARTFORGE_EFFECTS','DARTFORGE_NO_ENUM_PATTERN_IDENTITY']:base.pop(k,None)
assert run('vm',[str(sdk/'bin/dart'),'--packages='+str(package),str(entry)],base,60)==0
assert run('official-compile',[str(sdk/'bin/dart'),'compile','exe','--packages='+str(package),str(entry),'-o',str(out/'official')],base)==0
assert run('official',[str(out/'official')],base,60)==0
expected=(out/'vm.stdout').read_bytes();assert expected==(out/'official.stdout').read_bytes()
for enabled in [False,True]:
 label='on' if enabled else 'off';case=out/label;case.mkdir();env=base.copy();env.update(DARTFORGE_KEEP_IR='1',DARTFORGE_LINK_MAP='1')
 if not enabled:env['DARTFORGE_NO_ENUM_PATTERN_IDENTITY']='1'
 cmd=[str(producer),'compile-native',str(entry),'--sdk',str(sdk/'lib'),'--packages',str(package),'--optimize','--memory=tracing','--exceptions=check','--roots=shadow','--stack-trace=none','-o',str(case/'program')]
 exit=run(label+'-compile',cmd,env);row={'enabled':enabled,'compile_exit':exit};ledger['cases'].append(row);save()
 if exit:raise SystemExit('Actual Format compile failure:'+label)
 row['artifact_sha256']=sha(case/'program');row['map_sha256']=sha(case/'program.map');row['native_exit']=run(label,[str(case/'program')],env,60);got=(out/(label+'.stdout')).read_bytes();row['exact_VM']=got==expected;row['stderr_bytes']=(out/(label+'.stderr')).stat().st_size
 if not row['exact_VM']:row['first_difference']=next(({'line':i+1,'VM':a,'native':b} for i,(a,b) in enumerate(zip(expected.decode().splitlines(),got.decode().splitlines())) if a!=b),{'VM_bytes':len(expected),'native_bytes':len(got)})
 rows=[]
 for l in (case/'program.map').read_text().splitlines():
  m=re.match(r'^(0x[\dA-Fa-f]+)\s+(0x[\dA-Fa-f]+)\s+\[\s*\d+\]\s+(.*)$',l)
  if m and m[3].endswith('..convertFormatValue'):rows.append({'address':m[1],'bytes':int(m[2],16),'name':m[3]})
 row['convertFormatValue_symbols']=rows;ir=case/'program.df_tmp/program.ll';row['IR_path']=str(ir)
 if ir.exists():
  row['IR_sha256']=sha(ir);s=ir.read_text();matches=re.findall(r'define[^\n]*@[^\n]*\.\.convertFormatValue\([^\n]*\).*?\n\}',s,re.S);row['IR_function_matches']=len(matches)
  if len(matches)==1:(case/'convertFormatValue.ll').write_text(matches[0]+'\n');row['function_selector_calls']=len(re.findall(r'\bcall[^\n]*@dartforge_selector\(',matches[0]))
 save();print(label,'VM_EXACT',row['exact_VM'],'SIZE',rows,flush=True)
ledger.update({'inputs_unchanged':all(sha(Path(p))==h for p,h in inputs.items()),'ON_OFF_identical':(out/'on.stdout').read_bytes()==(out/'off.stdout').read_bytes(),'result':'PASS' if all(c['native_exit']==0 and c['exact_VM'] and c['stderr_bytes']==0 for c in ledger['cases']) else 'SEMANTIC_MISMATCH'});save();assert ledger['inputs_unchanged'];print('RESULT',ledger['result'],flush=True)

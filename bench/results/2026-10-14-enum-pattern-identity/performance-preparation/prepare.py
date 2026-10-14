from pathlib import Path
import os,subprocess,json,hashlib
root=Path.cwd();out=root/'target/enum-pattern-identity/format-performance';sdk=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk');producer=root/'target/native-receiver-producer/dartforge';pkg=Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui/examples/file_explorer/.dart_tool/package_config.json');source=out/'probe.dart';actual=Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui/lib/src/graphics/image/codecs/color/format.dart');sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();inputs={str(p):sha(p) for p in [producer,source,pkg,actual]};ledger={'inputs_before':inputs,'commands':[],'artifacts':{},'scope':'Compile-only paired real convertFormatValue throughput harness. Runs with rounds=1 below verify checksum, not performance. Timing not measured.'}
env=dict(os.environ,TMPDIR=str(out),DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang')
for k in ['DARTFORGE_NO_ENUM_PATTERN_IDENTITY','DARTFORGE_GC_STRESS','DARTFORGE_ARC_PREPARED','DARTFORGE_ARC_PREPARE','DARTFORGE_MEMORY','DARTFORGE_KEEP_IR','DARTFORGE_LINK_MAP']:env.pop(k,None)
def save():(out/'prepared.json').write_text(json.dumps(ledger,indent=2)+'\n')
def run(label,cmd,e):
 with (out/(label+'.stdout')).open('wb') as so,(out/(label+'.stderr')).open('wb') as se:r=subprocess.run(cmd,env=e,stdout=so,stderr=se,timeout=1200)
 ledger['commands'].append({'label':label,'command':cmd,'exit':r.returncode,'enum_optout':e.get('DARTFORGE_NO_ENUM_PATTERN_IDENTITY')});save();print(label,r.returncode,flush=True);assert r.returncode==0
run('vm',[str(sdk/'bin/dart'),'--packages='+str(pkg),str(source),'1'],env)
run('official-compile',[str(sdk/'bin/dart'),'compile','exe','--packages='+str(pkg),str(source),'-o',str(out/'official')],env)
for label in ['off','on']:
 e=env.copy()
 if label=='off':e['DARTFORGE_NO_ENUM_PATTERN_IDENTITY']='1'
 run(label+'-compile',[str(producer),'compile-native',str(source),'--sdk',str(sdk/'lib'),'--packages',str(pkg),'--optimize','--memory=tracing','--exceptions=check','--roots=shadow','--stack-trace=none','-o',str(out/label)],e)
for label in ['official','off','on']:
 run(label+'-check',[str(out/label),'1'],env);assert (out/(label+'-check.stdout')).read_bytes()==(out/'vm.stdout').read_bytes();assert (out/(label+'-check.stderr')).stat().st_size==0
 ledger['artifacts'][label]={'path':str(out/label),'sha256':sha(out/label)}
ledger['inputs_unchanged']=all(sha(Path(p))==h for p,h in inputs.items());assert ledger['inputs_unchanged'];ledger['result']='READY';ledger['expected_one_round_stdout']=(out/'vm.stdout').read_text();save();print('READY',flush=True)

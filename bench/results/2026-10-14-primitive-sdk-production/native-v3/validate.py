"""Compare frozen native profiles to actual SDK VM/AOT outputs; no timing."""
from pathlib import Path
import argparse,hashlib,json,os,subprocess
parser=argparse.ArgumentParser();parser.add_argument('--compiler',type=Path,required=True);parser.add_argument('--sha256',required=True);args=parser.parse_args()
root=Path.cwd();archive=root/'bench/results/2026-10-14-primitive-sdk-production/native-v3';work=root/'target/primitive-sdk-native-v3';work.mkdir(parents=True,exist_ok=True)
cli=args.compiler.resolve();sdk=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk');packages=Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui/examples/file_explorer/.dart_tool/package_config.json')
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert sha(cli)==args.sha256
env={k:v for k,v in os.environ.items() if not k.startswith('DARTFORGE_')};env.update(TMPDIR=str(work),DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang',LLVM_SYS_221_PREFIX='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64',DARTFORGE_SDK_LIB=str(sdk/'lib'),DARTFORGE_KEEP_IR='1',DARTFORGE_LINK_MAP='1')
cases=[('color',root/'bench/comparison/color_numeric.dart',2736,'DARTFORGE_NO_PRIMITIVE_MULTIPLY'),('multiply',root/'crates/emit_native/tests/fixtures/primitive_multiply.dart',230,'DARTFORGE_NO_PRIMITIVE_MULTIPLY'),('sdk-factory',root/'crates/emit_native/tests/fixtures/devirtualization/sdk_factory_returns.dart',5,'DARTFORGE_NO_SDK_ALLOCATION_PROOFS')]
inputs={str(p):sha(p) for p in [cli,sdk/'bin/dart',packages,*[c[1] for c in cases]]};report={'inputs':inputs,'environment':{k:v for k,v in env.items() if k.startswith('DARTFORGE_')},'scope':'Same frozen compiler per OFF/ON; official VM/AOT consumed outputs, native normal/GCstress. Static IR and executable bytes are separate from runtime performance.','cases':{}}
def save():(archive/'validation.json').write_text(json.dumps(report,indent=2)+'\n')
def run(label,command,destination,extra=None):
 destination.mkdir(parents=True,exist_ok=True);e=env.copy();e.update(extra or {});r=subprocess.run(command,env=e,stdout=(destination/(label+'.stdout')).open('wb'),stderr=(destination/(label+'.stderr')).open('wb'),timeout=900)
 result={'command':command,'exit':r.returncode,'stdout_sha256':sha(destination/(label+'.stdout')),'stderr_sha256':sha(destination/(label+'.stderr'))};return result
save()
for name,source,lines,flag in cases:
 folder=archive/name;target=work/name;target.mkdir(exist_ok=True);item={'source_sha256':sha(source),'ablation':flag,'expected_lines':lines,'profiles':{}};report['cases'][name]=item
 item['vm']=run('vm',[str(sdk/'bin/dart'),'--packages='+str(packages),str(source)],folder);save();assert item['vm']['exit']==0
 oracle=(folder/'vm.stdout').read_bytes();assert len(oracle.splitlines())==lines
 item['official-compile']=run('official-compile',[str(sdk/'bin/dart'),'compile','exe','--packages='+str(packages),str(source),'-o',str(target/'official')],folder);save();assert item['official-compile']['exit']==0
 item['official']=run('official',[str(target/'official')],folder);item['official']['exact']=item['official']['exit']==0 and (folder/'official.stdout').read_bytes()==oracle;save();assert item['official']['exact']
 for profile in ['off','on']:
  output=target/profile;output.mkdir(exist_ok=True);e={flag:'1'} if profile=='off' else ({'DARTFORGE_PRIMITIVE_MULTIPLY':'1'} if flag=='DARTFORGE_NO_PRIMITIVE_MULTIPLY' else {});binary=output/'program';command=[str(cli),'compile-native',str(source),'--sdk',str(sdk/'lib'),'--packages',str(packages),'--optimize','--memory=tracing','--exceptions=check','--roots=shadow','--stack-trace=none','-o',str(binary)];data={'compile':run('compile',command,folder/profile,e)};item['profiles'][profile]=data;save();assert data['compile']['exit']==0
  data['artifact']={'bytes':binary.stat().st_size,'sha256':sha(binary)}
  for stress in [False,True]:
   label='stress' if stress else 'normal';runtime=dict(e,**({'DARTFORGE_GC_STRESS':'1'} if stress else {}));data[label]=run(label,[str(binary)],folder/profile,runtime);data[label]['exact']=data[label]['exit']==0 and (folder/profile/(label+'.stdout')).read_bytes()==oracle;save();assert data[label]['exact'],(name,profile,label)
  data['ir']={str(p.relative_to(output)):{'sha256':sha(p),'bytes':p.stat().st_size} for p in output.rglob('*.ll')};save();print(name,profile,'exact',flush=True)
report['inputs_unchanged']=all(sha(Path(p))==h for p,h in inputs.items());save();assert report['inputs_unchanged']

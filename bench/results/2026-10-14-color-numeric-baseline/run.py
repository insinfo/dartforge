"""Exact ordinary color-kernel differential baseline, without timing claims."""
from pathlib import Path
import hashlib,json,os,subprocess
ROOT=Path.cwd();ARCHIVE=ROOT/'bench/results/2026-10-14-color-numeric-baseline';OUT=ROOT/'target/color-numeric-baseline';OUT.mkdir(parents=True,exist_ok=True)
SOURCE=ROOT/'bench/comparison/color_numeric.dart';FORMAT=Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui/lib/src/graphics/image/codecs/color/format.dart');SDK=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk');CLI=ROOT/'target/selector-getter-producer/dartforge';PACKAGES=Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui/examples/file_explorer/.dart_tool/package_config.json')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
inputs={str(p):sha(p) for p in [SOURCE,FORMAT,SDK/'bin/dart',CLI,PACKAGES]}
assert inputs[str(CLI)]=='0a876b5adb34707a38fc75d4a5f74da90d22fb26fc77cf9520c403b7d5e79d2f'
env={k:v for k,v in os.environ.items() if not k.startswith('DARTFORGE_')};env.update(TMPDIR=str(OUT),DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang',LLVM_SYS_221_PREFIX='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64',DARTFORGE_SDK_LIB=str(SDK/'lib'))
report={'inputs':inputs,'cases':19*12*12,'scope':'Actual unmodified dart_ui format.dart. Exact output bit fingerprints and error type; no timing or optimization benefit claim.','runs':{}}
def save():(ARCHIVE/'validation.json').write_text(json.dumps(report,indent=2)+'\n')
def run(name,command,extra=None):
 e=env.copy();e.update(extra or {});r=subprocess.run(command,env=e,stdout=(ARCHIVE/(name+'.stdout')).open('wb'),stderr=(ARCHIVE/(name+'.stderr')).open('wb'),timeout=600)
 report['runs'][name]={'command':command,'exit_code':r.returncode,'stdout_sha256':sha(ARCHIVE/(name+'.stdout')),'stderr_sha256':sha(ARCHIVE/(name+'.stderr'))};save();print(name,r.returncode,flush=True);return r.returncode
save()
run('vm',[str(SDK/'bin/dart'),'--packages='+str(PACKAGES),str(SOURCE)])
if run('official-compile',[str(SDK/'bin/dart'),'compile','exe','--packages='+str(PACKAGES),str(SOURCE),'-o',str(OUT/'official')])==0:run('official',[str(OUT/'official')])
if run('forge-compile',[str(CLI),'compile-native',str(SOURCE),'--sdk',str(SDK/'lib'),'--packages',str(PACKAGES),'--optimize','--memory=tracing','--exceptions=check','--roots=shadow','--stack-trace=none','-o',str(OUT/'forge')])==0:
 run('forge',[str(OUT/'forge')]);run('forge-stress',[str(OUT/'forge')],{'DARTFORGE_GC_STRESS':'1'})
 report['artifact']={'bytes':(OUT/'forge').stat().st_size,'sha256':sha(OUT/'forge')}
oracle=(ARCHIVE/'vm.stdout').read_bytes();report['exact_oracle_matches']={name:(ARCHIVE/(name+'.stdout')).exists() and (ARCHIVE/(name+'.stdout')).read_bytes()==oracle and report['runs'][name]['exit_code']==0 for name in ['official','forge','forge-stress']}
report['inputs_unchanged']=all(sha(Path(p))==h for p,h in inputs.items());save();print(json.dumps({'matches':report['exact_oracle_matches'],'inputs_unchanged':report['inputs_unchanged']},indent=2),flush=True)

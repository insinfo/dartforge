from pathlib import Path
import json,subprocess,os,hashlib,sys
shared=Path('/media/psf/ssd_dock/MyRustProjects/dartforge/target/current-desktop-validation');compiler=Path('/home/isaque/projects/dartforge/target/native-validation/release/dartforge-current-desktop-fixed');snapshot=sys.argv[1];fixture=Path(snapshot)/'crates/cli/tests/fixtures/jit_main_arguments.dart';sdk='/opt/dart/3.6.2/dart-sdk/lib';rows=[]
env=os.environ.copy();env.update(DARTFORGE_LLVM_DIR='/opt/llvm-22.1.8',DARTFORGE_SDK_LIB=str(shared/'invalid-sdk'));env['PATH']='/opt/llvm-22.1.8/bin:'+env['PATH'];env.pop('DARTFORGE_SDK_DLL',None);env.pop('DART_SDK',None)
for mode in ['run','reload','restart']:
 command=[str(compiler),'run' if mode=='run' else 'reload','--sdk',sdk,str(fixture)]
 if mode!='run':command.append('--once')
 if mode=='restart':command.append('--restart')
 command+=['--','--sdk','application-owned','--'];p=subprocess.run(command,env=env,capture_output=True,text=True)
 rows.append(dict(mode=mode,command=command,compiler_sha256=hashlib.sha256(compiler.read_bytes()).hexdigest(),exit=p.returncode,stdout=p.stdout,stderr=p.stderr));(shared/'linux-sdk-modes.json').write_text(json.dumps(rows,indent=2)+'\n');print(mode,p.returncode,flush=True)
 assert p.returncode==0 and p.stdout=='["--sdk","application-owned","--"]\n',rows[-1]

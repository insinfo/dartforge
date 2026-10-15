"""Compare the repository SDK's absent-package behavior in three actual CLI modes."""
from pathlib import Path
import hashlib,json,os,shutil,subprocess
root=Path.cwd();out=Path(__file__).resolve().parent;work=root/'target/sdk-package-uri-null/product';work.mkdir(parents=True,exist_ok=True)
source=work/'main.dart';shutil.copyfile(root/'crates/cli/tests/fixtures/jit_package_absence.dart',source)
stale=work/'stale.json';stale.write_text('{"configVersion":2,"packages":[]}\n')
compiler=root/'target/jit-package-config-producer-v2/dartforge';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
env=os.environ.copy();env['DARTFORGE_NATIVE_SDK']=str(root/'native_sdk');env['DARTFORGE_NATIVE_CACHE']=str(root/'target/sdk-package-uri-null/cache-v2');env['DARTFORGE_PACKAGE_CONFIG']=str(stale)
report={'compiler_sha256':sha(compiler),'sdk_patch_sha256':sha(root/'native_sdk/isolate/isolate_patch.dart'),'source_sha256':sha(source),'inherited_config':str(stale),'commands':[]}
commands=[('vm',['/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart',str(source)])]
for mode in ['run','reload','restart']:
 command=[str(compiler),'run' if mode=='run' else 'reload','--sdk','/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib',str(source)]
 if mode!='run':command.append('--once')
 if mode=='restart':command.append('--restart')
 commands.append((mode,command))
for name,command in commands:
 with (out/(name+'.stdout')).open('wb') as stdout,(out/(name+'.stderr')).open('wb') as stderr:r=subprocess.run(command,cwd=work,env=env,stdout=stdout,stderr=stderr)
 report['commands'].append({'name':name,'command':command,'exit_code':r.returncode,'stdout':(out/(name+'.stdout')).read_text(),'stderr_sha256':sha(out/(name+'.stderr'))})
 (out/'validation.json').write_text(json.dumps(report,indent=2)+'\n');print(name,r.returncode,flush=True)
report['all_exact']=all(c['exit_code']==0 and c['stdout']==report['commands'][0]['stdout'] for c in report['commands']);report['sdk_unchanged']=report['sdk_patch_sha256']==sha(root/'native_sdk/isolate/isolate_patch.dart')
(out/'validation.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'all_exact':report['all_exact'],'sdk_unchanged':report['sdk_unchanged']}),flush=True)
raise SystemExit(0 if report['all_exact'] and report['sdk_unchanged'] else 1)

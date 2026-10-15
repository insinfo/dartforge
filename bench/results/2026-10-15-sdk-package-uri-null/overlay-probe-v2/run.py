"""Compare an isolated SDK candidate with the VM without changing repository SDK bytes."""
from pathlib import Path
import hashlib,json,os,shutil,subprocess
root=Path.cwd();out=Path(__file__).resolve().parent;work=root/'target/sdk-package-uri-null';overlay=work/'native_sdk-v2'
assert not overlay.exists(), 'Choose a fresh overlay directory'
for path in (root/'native_sdk').rglob('*'):
 target=overlay/path.relative_to(root/'native_sdk')
 if path.is_dir():target.mkdir(parents=True,exist_ok=True)
 elif path.is_file():target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(path,target)
shutil.copyfile(work/'isolate_patch.dart',overlay/'isolate/isolate_patch.dart')
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
env=os.environ.copy();env['DARTFORGE_NATIVE_SDK']=str(overlay);env['DARTFORGE_NATIVE_CACHE']=str(work/'cache-v2')
compiler=root/'target/jit-package-config-producer-v2/dartforge'
report={'scope':'Frozen compiler with isolated candidate SDK overlay; repository SDK unchanged','compiler_sha256':sha(compiler),'overlay_patch_sha256':sha(overlay/'isolate/isolate_patch.dart'),'repository_patch_sha256':sha(root/'native_sdk/isolate/isolate_patch.dart'),'source_sha256':sha(work/'probe.dart'),'overlay':str(overlay),'commands':[]}
for name,command in [('vm',['/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart',str(work/'probe.dart')]),('jit',[str(compiler),'run','--sdk','/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib',str(work/'probe.dart')])]:
 with (out/(name+'.stdout')).open('wb') as stdout,(out/(name+'.stderr')).open('wb') as stderr:r=subprocess.run(command,cwd=work,env=env,stdout=stdout,stderr=stderr)
 report['commands'].append({'name':name,'command':command,'exit_code':r.returncode,'stdout':(out/(name+'.stdout')).read_text(),'stderr_sha256':sha(out/(name+'.stderr'))})
 (out/'validation.json').write_text(json.dumps(report,indent=2)+'\n');print(name,r.returncode,flush=True)
report['exact']=all(c['exit_code']==0 for c in report['commands']) and report['commands'][0]['stdout']==report['commands'][1]['stdout']
report['repository_unchanged']=report['repository_patch_sha256']==sha(root/'native_sdk/isolate/isolate_patch.dart')
(out/'validation.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'exact':report['exact'],'repository_unchanged':report['repository_unchanged']}),flush=True)
raise SystemExit(0 if report['exact'] and report['repository_unchanged'] else 1)

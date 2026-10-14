"""Compare native root overrides with installed Pub using local packages."""
from pathlib import Path
from urllib.parse import urljoin, urlparse, unquote
import hashlib
import json
import os
import subprocess
root=Path.cwd();out=root/'bench/results/2026-10-14-pub-root-overrides';work=root/'target/pub-overrides-status'
compiler=work/'dartforge';sdk=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart')
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
source_paths=list((root/'crates/cli/src/pub_get').glob('*.rs'))+[root/'crates/cli/src/pub_get.rs']
sources={str(p):sha(p) for p in source_paths}
env=dict(os.environ,TMPDIR=str(root/'target/tmp-p8-runtime'))
rows=[]
def normalize(app):
 lock=json.loads(subprocess.check_output(['python3','-c',"import json,sys,yaml;print(json.dumps(yaml.safe_load(open(sys.argv[1]))))",str(app/'pubspec.lock')]))
 for p in lock['packages'].values():
  desc=p['description'];path=Path(desc['path'])
  desc['path']=str((app/path).resolve()) if desc['relative'] else str(path.resolve())
  desc.pop('relative')
 config=json.loads((app/'.dart_tool/package_config.json').read_text())
 packages={p['name']:{'root':unquote(urlparse(urljoin((app/'.dart_tool/package_config.json').as_uri(),p['rootUri'])).path),'language':p['languageVersion'],'packageUri':p['packageUri']} for p in config['packages']}
 return {'lock':lock,'packages':packages}
def execute(label,expected):
 app=work/'app';reports={}
 for mode,binary,args in [('sdk',sdk,[]),('native',compiler,[])]:
  command=[str(binary),'pub','get','--offline','--directory',str(app)]
  p=subprocess.run(command,env=env,capture_output=True,timeout=120)
  (out/(label+'-'+mode+'.stdout')).write_bytes(p.stdout);(out/(label+'-'+mode+'.stderr')).write_bytes(p.stderr)
  assert (p.returncode==0)==expected,(label,mode,p.stderr)
  rows.append({'label':label,'mode':mode,'command':command,'exit':p.returncode})
  if expected:
   reports[mode]=normalize(app)
   for name in ['pubspec.lock','.dart_tool/package_config.json']:
    (out/(label+'-'+mode+'-'+Path(name).name)).write_bytes((app/name).read_bytes())
 if expected: assert reports['sdk']==reports['native'],reports
 return reports
(work/'app/pubspec_overrides.yaml').unlink(missing_ok=True)
base=execute('root-map',True)
assert set(base['native']['lock']['packages'])=={'a','b','c'}
assert base['native']['lock']['packages']['b']['dependency']=='direct overridden'
(work/'app/pubspec_overrides.yaml').write_text('dependency_overrides:\n  b:\n    path: ../b\n')
side=execute('sidecar-replaces',True)
assert set(side['native']['lock']['packages'])=={'a','b'}
(work/'b/pubspec.yaml').write_text('name: b\nversion: 2.0.0\nenvironment:\n  sdk: ^4.0.0\n')
execute('sdk-bound-refused',False)
(work/'b/pubspec.yaml').write_text('name: b\nversion: 2.0.0\nenvironment:\n  sdk: ^3.6.0\n')
execute('offline-repeat',True)
assert all(sha(p)==h for p,h in sources.items())
(out/'validation.json').write_text(json.dumps({'compiler':str(compiler),'compiler_sha256':sha(compiler),'sdk_sha256':sha(sdk),'sources':sources,'runs':rows,'complete':True,'normalized_success_reports_exact':True,'scope':'Actual CLI vs SDK3.6.2 Pub, local path overrides replace hosted transitive version constraints and include otherwise unused overrides; sidecar replaces whole map. SDK-incompatible package still rejected. No timing claim; git/sdk/workspaces remain unsupported.'},indent=2)+'\n')
print('Native and SDK success graphs exact; SDK bound refused by both.')

from pathlib import Path
import os,subprocess,json,hashlib,yaml,argparse,shutil
from urllib.parse import urljoin,urlparse
from urllib.request import url2pathname
parser=argparse.ArgumentParser();parser.add_argument('--compiler',type=Path,required=True);args=parser.parse_args()
root=Path.cwd(); out=root/'bench/results/2026-10-14-native-pub-get';work=root/'target/native-provenance/native-pub-get-validation'; work.mkdir(parents=True,exist_ok=True)
compiler=args.compiler.resolve(); dart=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart')
ledger={'prototype':False,'delegates_to_dart':False,'timing_measured':False,'compiler':str(compiler),'compiler_sha256':hashlib.sha256(compiler.read_bytes()).hexdigest(),'dart_sdk_executable':str(dart),'dart_sdk_executable_sha256':hashlib.sha256(dart.read_bytes()).hexdigest(),'runs':[]}
def package(path,text):
 path.mkdir(parents=True,exist_ok=True);(path/'lib').mkdir(exist_ok=True);(path/'pubspec.yaml').write_text(text)
def run(label,command,cache):
 env=os.environ.copy();env['PUB_CACHE']=str(cache);env['TMPDIR']=str(root/'target/tmp-p8-runtime');env.pop('PUB_HOSTED_URL',None)
 r=subprocess.run(list(map(str,command)),env=env,capture_output=True,timeout=300)
 (out/(label+'.stdout')).write_bytes(r.stdout);(out/(label+'.stderr')).write_bytes(r.stderr)
 ledger['runs'].append({'label':label,'command':list(map(str,command)),'PUB_CACHE':str(cache),'exit':r.returncode,'stdout_sha256':hashlib.sha256(r.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(r.stderr).hexdigest()})
 (out/'validation.json').write_text(json.dumps(ledger,indent=2)+'\n');print(label,r.returncode,flush=True);return r
version=run('dart-sdk-version',[dart,'--version'],work/'official-cache');assert version.returncode==0 and b'3.6.2' in version.stdout+version.stderr
sdk='environment:\n  sdk: ^3.6.0\n'
package(work/'shared', 'name: local_dep\nversion: 1.2.3\n'+sdk)
(work/'shared/lib/local_dep.dart').write_text('int answer() => 42;\n')
for kind in ['native','official']:
 package(work/('path-'+kind),'name: local_app\n'+sdk+'dependencies:\n  local_dep:\n    path: ../shared\n')
 package(work/('hosted-'+kind),'name: hosted_app\n'+sdk+'dependencies:\n  http: ^1.2.2\n  collection: ^1.19.0\n  pub_semver: ^2.1.4\n')
 package(work/('conflict-'+kind),'name: conflict_app\n'+sdk+'dependencies:\n  collection: ^1.19.0\n  constrained:\n    path: ../constrained\n')
package(work/'constrained','name: constrained\nversion: 1.0.0\n'+sdk+'dependencies:\n  collection: "<1.19.0"\n')
for scenario in ['path','hosted','conflict']:
 for kind in ['native','official']:
  directory=work/(scenario+'-'+kind);cache=work/(kind+'-cache')
  command=[compiler,'pub','get','--directory',directory] if kind=='native' else [dart,'pub','get','--directory',directory]
  if scenario=='path':command+=['--offline']
  result=run(scenario+'-'+kind,command,cache)
  assert (result.returncode!=0)==(scenario=='conflict'),result.stderr.decode()
 if scenario!='conflict':
  a=work/(scenario+'-native');b=work/(scenario+'-official')
  native=yaml.safe_load((a/'pubspec.lock').read_text());official=yaml.safe_load((b/'pubspec.lock').read_text())
  def lock_normalize(lock):
   out={}
   for n,p in lock['packages'].items():
    d=p['description'];out[n]={'source':p['source'],'dependency':p['dependency'],'version':p['version']}
    if p['source']=='hosted':out[n]['description']=d
    else:out[n]['path']=Path(d['path']).name
   return {'packages':out,'sdks':lock['sdks']}
  assert lock_normalize(native)==lock_normalize(official),(native,official)
  def config(directory):
   v=json.loads((directory/'.dart_tool/package_config.json').read_text())
   return {p['name']:{'packageUri':p.get('packageUri'),'languageVersion':p.get('languageVersion'),'root_directory':Path(p['rootUri']).name if p['rootUri'].startswith('file:') else ('shared' if p['name']=='local_dep' else p['name'])} for p in v['packages']}
  # Compare language/package roots by package identity; absolute URI spelling is valid but differs.
  def shape(directory):
   v=json.loads((directory/'.dart_tool/package_config.json').read_text());return {p['name']:(p['packageUri'],p['languageVersion']) for p in v['packages']}
  assert shape(a)==shape(b),(shape(a),shape(b))
  ledger[scenario+'_sdk_lock_and_config_equivalent']=True
  if scenario=='path':
   program="import 'package:local_dep/local_dep.dart'; void main() { print(answer()); }\n"
  else:
   program="""import 'dart:convert';
import 'package:collection/collection.dart';
import 'package:http/http.dart';
import 'package:pub_semver/pub_semver.dart';
void main() {
  print(jsonEncode({'same': const ListEquality<int>().equals([1,2],[1,2]),
    'uri': Request('GET',Uri.parse('https://example.com')).url.toString(),
    'pubSemver': [
      Version.parse('1.0.0+2') < Version.parse('1.0.0+10'),
      VersionConstraint.parse('1.0.0+1').allows(Version.parse('1.0.0+2')),
      VersionConstraint.parse('>2.0.0-alpha <2.0.0').allows(Version.parse('2.0.0-beta')),
      VersionConstraint.parse('>=1.0.0 <2.0.0').allows(Version.parse('2.0.0-beta')),
      VersionConstraint.parse('<2.0.0+1').allows(Version.parse('2.0.0-beta'))
    ]}));
}
"""
  vm_outputs=[]
  for directory in [a,b]:
   (directory/'main.dart').write_text(program)
   execution=run(scenario+'-'+directory.name.rsplit('-',1)[-1]+'-package-config-vm',[dart,'--packages='+str(directory/'.dart_tool/package_config.json'),directory/'main.dart'],work/'official-cache')
   assert execution.returncode==0 and not execution.stderr,execution.stderr.decode()
   vm_outputs.append(execution.stdout)
  assert vm_outputs[0]==vm_outputs[1]
  if scenario=='hosted':assert json.loads(vm_outputs[0])['pubSemver']==[True,False,True,False,True]
  ledger[scenario+'_generated_config_executes_exact_sdk']=True

  for directory in [a,b]:
   config_path=directory/'.dart_tool/package_config.json'
   for entry in json.loads(config_path.read_text())['packages']:
    package_root=Path(url2pathname(urlparse(urljoin(config_path.as_uri(),entry['rootUri'])).path))
    assert package_root.is_dir(),package_root
    assert yaml.safe_load((package_root/'pubspec.yaml').read_text())['name']==entry['name']
  for directory in [a,b]:
   for artifact in ['pubspec.lock','.dart_tool/package_config.json']:
    path=directory/artifact
    archive=out/directory.name/artifact;archive.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(path,archive)
    ledger.setdefault('artifacts',[]).append({'path':str(path),'sha256':hashlib.sha256(path.read_bytes()).hexdigest()})
  before={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in [a/'pubspec.lock',a/'.dart_tool/package_config.json']}
  result=run(scenario+'-native-offline',[compiler,'pub','get','--directory',a,'--offline'],work/'native-cache');assert result.returncode==0,result.stderr.decode()
  assert before=={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in [a/'pubspec.lock',a/'.dart_tool/package_config.json']}
ledger['hosted_metadata']=[{'path':str(p),'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in sorted((work/'native-cache/dartforge').rglob('*.json'))]
ledger['compiler_unchanged']=hashlib.sha256(compiler.read_bytes()).hexdigest()==ledger['compiler_sha256'];assert ledger['compiler_unchanged'];ledger['complete']=True;(out/'validation.json').write_text(json.dumps(ledger,indent=2)+'\n')

"""Exercise runtime package resources through a frozen CLI in ordinary project CWD."""
from pathlib import Path
from datetime import datetime,timezone
import argparse,hashlib,json,os,subprocess
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--producer',type=Path,required=True)
parser.add_argument('--producer-sha256',required=True)
args=parser.parse_args()
root=Path(__file__).resolve().parents[3]
archive=Path(__file__).resolve().parent
run_id=datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')
evidence=archive/'package-runs'/run_id
work=root/'target/jit-file-explorer-package-gate'/run_id
project=work/'project'
for directory in [evidence,project/'bin',project/'.dart_tool',work/'dependency/lib',work/'tmp']:
 directory.mkdir(parents=True,exist_ok=False)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(args.producer)==args.producer_sha256
source=root/'crates/cli/tests/fixtures/jit_package_resource.dart'
entry=project/'bin/main.dart';entry.write_bytes(source.read_bytes())
(work/'dependency/lib/resource.txt').write_text('selected package resource\n')
config=project/'.dart_tool/package_config.json'
config.write_text(json.dumps({'configVersion':2,'packages':[{'name':'resource_package','rootUri':'../../dependency/','packageUri':'lib/','languageVersion':'3.0'}]},indent=2)+'\n')
env={k:v for k,v in os.environ.items() if not k.startswith('DARTFORGE_')}
env.update(TMPDIR=str(work/'tmp'),DARTFORGE_SDK_LIB='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib',DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang',LLVM_SYS_221_PREFIX='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64',SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk')
record={'status':'running','producer':str(args.producer),'producer_sha256':sha(args.producer),'source_sha256':sha(source),'config_sha256':sha(config),'cwd':str(project),'commands':[]}
validation=evidence/'validation.json'
def save():validation.write_text(json.dumps(record,indent=2)+'\n')
def execute(label,command,environment):
 print(label,flush=True)
 output=subprocess.run(command,cwd=project,env=environment,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=900)
 (evidence/(label+'.stdout')).write_bytes(output.stdout);(evidence/(label+'.stderr')).write_bytes(output.stderr)
 item={'label':label,'command':command,'exit':output.returncode,'exact_resource':output.stdout==b'selected package resource\n','stdout_sha256':hashlib.sha256(output.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(output.stderr).hexdigest()}
 record['commands'].append(item);save()
 if output.returncode or not item['exact_resource']:
  record['status']='failed';save();raise SystemExit(1)
save()
execute('vm',['/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart','bin/main.dart'],env)
for explicit in [False,True]:
 for mode in ['run','reload','restart']:
  command=[str(args.producer),'run' if mode=='run' else 'reload']
  if mode!='run':command+=['bin/main.dart','--once']
  command+=['--sdk',env['DARTFORGE_SDK_LIB']]
  if explicit:command+=['--packages','.dart_tool/package_config.json']
  if mode=='run':command+=['bin/main.dart']
  if mode=='restart':command+=['--restart']
  execute(mode+('-explicit' if explicit else '-discovered'),command,dict(env,DARTFORGE_PACKAGE_CONFIG=str(work/'stale.json')))
record.update(status='passed',inputs_stable=sha(args.producer)==args.producer_sha256 and sha(source)==record['source_sha256'] and sha(entry)==record['source_sha256'] and sha(config)==record['config_sha256'])
save();print(validation,flush=True)

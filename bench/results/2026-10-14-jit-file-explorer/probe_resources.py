"""Compare real package/font lookup with an isolated runtime config diagnostic."""
from pathlib import Path
from datetime import datetime,timezone
import argparse,hashlib,json,os,subprocess
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--producer',type=Path,required=True)
parser.add_argument('--producer-sha256',required=True)
parser.add_argument('--launch',type=Path,required=True)
parser.add_argument('--diagnostic-env-comparison',action='store_true')
args=parser.parse_args()
archive=Path(__file__).resolve().parent
r=json.loads(args.launch.read_text())
source=archive/'resource_probe.dart'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(args.producer)==args.producer_sha256
run=archive/'resource-runs'/datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')
run.mkdir(parents=True)
config=Path(r['example_copy'])/'.dart_tool/package_config.json'
env={k:v for k,v in os.environ.items() if not k.startswith('DARTFORGE_') and k!='DART_UI_FONTS'}
env.update(TMPDIR=str(Path(r['work'])/'tmp'),DARTFORGE_SDK_LIB='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib',DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang',LLVM_SYS_221_PREFIX='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64',SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk')
record={'producer_sha256':sha(args.producer),'source_sha256':sha(source),'config_sha256':sha(config),'cwd':r['example_copy'],'diagnostic_env_comparison':args.diagnostic_env_comparison,'commands':[],'status':'running'}
path=run/'validation.json'
def save():path.write_text(json.dumps(record,indent=2)+'\n')
save()
commands=[('vm',['/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart','--packages='+str(config),str(source)],env),('jit',[str(args.producer),'run','--sdk',env['DARTFORGE_SDK_LIB'],'--packages',str(config),str(source)],env)]
if args.diagnostic_env_comparison:
 commands.append(('jit-explicit-runtime-config',commands[1][1],dict(env,DARTFORGE_PACKAGE_CONFIG=str(config))))
for label,command,environment in commands:
 print(label,flush=True)
 result=subprocess.run(command,cwd=r['example_copy'],env=environment,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=900)
 (run/(label+'.stdout')).write_bytes(result.stdout);(run/(label+'.stderr')).write_bytes(result.stderr)
 item={'label':label,'command':command,'exit':result.returncode,'runtime_config':environment.get('DARTFORGE_PACKAGE_CONFIG'),'stdout_sha256':hashlib.sha256(result.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(result.stderr).hexdigest()}
 try:item['result']=json.loads(result.stdout)
 except (ValueError,UnicodeDecodeError):item['result']=None
 record['commands'].append(item);save()
 if result.returncode: record['status']='failed';save();raise SystemExit(result.returncode)
vm=record['commands'][0]['result']
assert vm and vm['phosphorIconFontLoaded'] and vm['fluentIconFontLoaded']
record['native_equals_vm']=record['commands'][1]['result']==vm
if args.diagnostic_env_comparison:
 record['diagnostic_equals_vm']=record['commands'][2]['result']==vm
 record['status']='diagnosis-confirmed' if not record['native_equals_vm'] and record['diagnostic_equals_vm'] else 'unexpected'
else:record['status']='passed' if record['native_equals_vm'] else 'failed'
record['inputs_stable']=sha(args.producer)==args.producer_sha256 and sha(source)==record['source_sha256'] and sha(config)==record['config_sha256']
save();print(path,flush=True)
raise SystemExit(0 if record['status'] in ('passed','diagnosis-confirmed') else 1)

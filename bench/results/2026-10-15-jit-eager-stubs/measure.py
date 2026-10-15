"""Alternate fresh-process OFF/ON linking of one saved real application IR.

Run only in a coordinated quiet window. IR emission is a separate probe command.
"""
from pathlib import Path
from datetime import datetime,timezone
import argparse,hashlib,json,os,statistics,subprocess
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--probe',type=Path,required=True)
parser.add_argument('--probe-sha256',required=True)
parser.add_argument('--ir',type=Path,required=True)
parser.add_argument('--ir-sha256',required=True)
parser.add_argument('--sdk-dll',type=Path,required=True)
parser.add_argument('--sdk-sha256',required=True)
parser.add_argument('--pairs',type=int,default=3)
args=parser.parse_args()
args.probe=args.probe.resolve();args.ir=args.ir.resolve();args.sdk_dll=args.sdk_dll.resolve()
assert args.pairs>=2
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
for path,expected in [(args.probe,args.probe_sha256),(args.ir,args.ir_sha256),(args.sdk_dll,args.sdk_sha256)]:assert sha(path)==expected
base=Path(__file__).resolve().parent;root=base.parents[2]
run_id=datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ');archive=base/'runs'/run_id;archive.mkdir(parents=True)
work=root/'target/jit-eager-stubs-measure'/run_id;work.mkdir(parents=True)
env={k:v for k,v in os.environ.items() if not k.startswith('DARTFORGE_')};env['TMPDIR']=str(work)
record={'status':'running','probe':str(args.probe),'probe_sha256':args.probe_sha256,'ir':str(args.ir),'ir_sha256':args.ir_sha256,'sdk_dll':str(args.sdk_dll),'sdk_sha256':args.sdk_sha256,'runner_sha256':sha(Path(__file__)),'pairs':args.pairs,'processes':[],'scope':'Fresh processes; identical emitted IR and SDK DLL. No app main/UI execution; emission excluded from link comparison.'}
validation=archive/'validation.json'
def save():validation.write_text(json.dumps(record,indent=2)+'\n')
def execute(label,mode,command):
 print(label,flush=True)
 try:result=subprocess.run(command,env=dict(env,DARTFORGE_JIT_EAGER_STUBS=mode),cwd=root,capture_output=True,timeout=1200)
 except subprocess.TimeoutExpired as error:
  stdout=error.stdout or b'';stderr=error.stderr or b''
  (archive/(label+'.stdout')).write_bytes(stdout);(archive/(label+'.stderr')).write_bytes(stderr)
  record['processes'].append({'label':label,'mode':mode,'command':command,'exit':None,'timeout_seconds':1200,'stdout_sha256':hashlib.sha256(stdout).hexdigest(),'stderr_sha256':hashlib.sha256(stderr).hexdigest()})
  record['status']='failed';save();raise SystemExit(1)
 (archive/(label+'.stdout')).write_bytes(result.stdout);(archive/(label+'.stderr')).write_bytes(result.stderr)
 item={'label':label,'mode':mode,'command':command,'exit':result.returncode,'stdout_sha256':hashlib.sha256(result.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(result.stderr).hexdigest()}
 try:item['result']=json.loads(result.stdout)
 except (ValueError,UnicodeDecodeError):item['result']=None
 record['processes'].append(item);save()
 if result.returncode or not item['result'] or item['result']['eager_stubs']!=mode:
  record['status']='failed';save();raise SystemExit(1)
 return item['result']
save()
for mode in ['0','1']:assert execute('small-'+mode,mode,[str(args.probe),'small'])['synthetic_result']==10
for pair in range(args.pairs):
 for mode in (['0','1'] if pair%2==0 else ['1','0']):execute(f'large-{pair}-{mode}',mode,[str(args.probe),'link',str(args.ir),str(args.sdk_dll)])
values={mode:[p['result']['total_ms'] for p in record['processes'] if p['label'].startswith('large-') and p['mode']==mode] for mode in ['0','1']}
record['inputs_stable']=all(sha(path)==expected for path,expected in [(args.probe,args.probe_sha256),(args.ir,args.ir_sha256),(args.sdk_dll,args.sdk_sha256)])
assert record['inputs_stable']
record.update(status='passed',median_total_ms={mode:statistics.median(v) for mode,v in values.items()},median_off_over_on=statistics.median(values['0'])/statistics.median(values['1']))
save();print(validation)

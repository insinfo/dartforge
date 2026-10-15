#!/usr/bin/env python3
"""Archive small execution and real File Explorer IR emission, without GUI startup."""
from pathlib import Path
import argparse, datetime, hashlib, json, os, subprocess
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--producer',type=Path,required=True)
a=parser.parse_args()
producer=json.loads(a.producer.read_text())
assert producer['status']=='passed'
root=Path(__file__).resolve().parents[3]
base=Path(__file__).resolve().parent
stamp=datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')
archive=base/'emission'/stamp;archive.mkdir(parents=True)
work=root/'target/jit-eager-stubs-ir'/stamp;work.mkdir(parents=True)
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
entry=Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui/examples/file_explorer/bin/file_explorer.dart')
config=entry.parents[1]/'.dart_tool/package_config.json'
sdk=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib')
inputs=[entry,config,Path(producer['probe']),Path(producer['emit_probe'])]
assert sha(producer['probe'])==producer['probe_sha256']
assert sha(producer['emit_probe'])==producer['emit_probe_sha256']
record={'status':'running','producer_manifest':str(a.producer.resolve()),'producer_manifest_sha256':sha(a.producer),'inputs':{str(p):sha(p) for p in inputs},'processes':[],'scope':'Small actual synthetic execution OFF/ON, then pristine ordinary File Explorer IR emission only. No large linking timings or application main execution.'}
manifest=archive/'validation.json'
env=dict(os.environ,TMPDIR=str(work))
def save():manifest.write_text(json.dumps(record,indent=2)+'\n')
def run(label,argv,mode):
 env['DARTFORGE_JIT_EAGER_STUBS']=mode
 print(label,flush=True)
 with (archive/(label+'.stdout')).open('w') as out,(archive/(label+'.stderr')).open('w') as err:r=subprocess.run(argv,env=env,cwd=root,stdout=out,stderr=err)
 item={'label':label,'command':argv,'exit':r.returncode,'stdout_sha256':sha(archive/(label+'.stdout')),'stderr_sha256':sha(archive/(label+'.stderr'))}
 try:item['result']=json.loads((archive/(label+'.stdout')).read_text())
 except ValueError:item['result']=None
 record['processes'].append(item);save()
 if r.returncode or item['result'] is None:
  record['status']='failed';save();raise SystemExit(1)
 return item['result']
save();print(archive,flush=True)
for mode in ['0','1']:
 result=run('small-'+mode,[producer['probe'],'small'],mode)
 assert result['synthetic_result']==10 and result['eager_stubs']==mode
saved=work/'file-explorer.ll'
result=run('emit',[producer['emit_probe'],'emit',str(entry),str(sdk),str(config),str(saved)],'0')
record['saved_ir']=str(saved);record['saved_ir_sha256']=sha(saved)
record['sdk_dll']=result['sdk_dll'];record['sdk_dll_sha256']=sha(result['sdk_dll'])
record['inputs_unchanged']=all(sha(p)==record['inputs'][str(p)] for p in inputs)
record['status']='passed' if record['inputs_unchanged'] else 'failed';save();print(manifest,flush=True)
raise SystemExit(0 if record['status']=='passed' else 1)

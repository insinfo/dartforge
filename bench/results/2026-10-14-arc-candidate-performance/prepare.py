"""Prepare paired ARC artifacts without making timing claims."""
from pathlib import Path
import argparse,hashlib,json,os,subprocess
parser=argparse.ArgumentParser()
parser.add_argument('--compiler',type=Path)
parser.add_argument('--output-dir',type=Path)
parser.add_argument('--work-dir',type=Path)
parser.add_argument('--before-artifacts',type=Path)
parser.add_argument('--before-metadata-bytes',type=int,default=24)
args=parser.parse_args()
root=Path.cwd();out=(args.output_dir or root/'bench/results/2026-10-14-arc-candidate-performance').resolve();work=(args.work_dir or root/'target/arc-candidate-performance').resolve()
out.mkdir(parents=True,exist_ok=True);work.mkdir(parents=True,exist_ok=True)
producer=(args.compiler or root/'target/candidate-platform-producer/dartforge').resolve()
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
old=json.loads((root/'bench/results/2026-10-14-current-native-status/prepared.json').read_text())
source=root/'bench/comparison/bench.dart'
assert sha(source)==old['inputs'][str(source)]
before=json.loads(args.before_artifacts.read_text()) if args.before_artifacts else None
a={'source':str(source),'source_sha256':sha(source),'compiler':str(producer),'compiler_sha256':sha(producer),'old_producer_sha256':before['compiler_sha256'] if before else old['compiler_sha256'],'old_metadata_bytes':args.before_metadata_bytes,'new_metadata_bytes':32,'artifacts':{},'runs':[],'complete':False}
for key in ['legacy-arc','prepared-arc','official']:
 v=before['artifacts']['old-official' if key=='official' else 'new-'+key] if before else old['artifacts'][key+'/bench'];assert sha(v['path'])==v['sha256'];a['artifacts']['old-'+key]=v
base={k:v for k,v in os.environ.items() if not k.startswith('DARTFORGE_')}
base.update(TMPDIR=str(root/'target/tmp-p8-runtime'),DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang',DARTFORGE_ARC_CHECK='0',DARTFORGE_ARC_PORT_PRUNING='1',DARTFORGE_FACTORY_RECEIVER_PROVENANCE='1',DARTFORGE_NO_PLATFORM_SPECIALIZATION='1')
for mode in ['legacy-arc','prepared-arc']:
 path=work/mode;env=dict(base,DARTFORGE_ARC_PREPARED='1' if mode=='prepared-arc' else '0')
 command=[str(producer),'compile-native',str(source),'--sdk',old['sdk']+'/lib','--optimize','--memory=arc','--exceptions=check','--roots=shadow','--stack-trace=none','-o',str(path)]
 p=subprocess.run(command,env=env,capture_output=True,timeout=1200)
 (out/(mode+'-compile.stdout')).write_bytes(p.stdout);(out/(mode+'-compile.stderr')).write_bytes(p.stderr)
 a['runs'].append({'command':command,'env':{k:v for k,v in env.items() if k.startswith('DARTFORGE_')},'exit':p.returncode})
 assert p.returncode==0,p.stderr.decode()
 a['artifacts']['new-'+mode]={'path':str(path),'sha256':sha(path),'bytes':path.stat().st_size}
 print(mode,'compiled',flush=True)
a['complete']=True;a['compiler_stable']=sha(producer)==a['compiler_sha256'];a['source_stable']=sha(source)==a['source_sha256']
(out/'prepared.json').write_text(json.dumps(a,indent=2)+'\n')

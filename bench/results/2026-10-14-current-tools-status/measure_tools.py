from pathlib import Path
import argparse,subprocess,os,json,hashlib,time,statistics,resource
parser=argparse.ArgumentParser();parser.add_argument('--quiet-window-authorized',action='store_true');args=parser.parse_args()
assert args.quiet_window_authorized,'Timing requires root quiet-window grant'
root=Path.cwd();out=root/'bench/results/2026-10-14-current-tools-status/timing';out.mkdir(exist_ok=True)
native=root/'target/current-toolchain-status/dartforge-native';dart=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart')
project=Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui/examples/file_explorer');files=sorted((dart.parent.parent/'lib').rglob('*.dart'))
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
ledger={'compiler_sha256':sha(native),'sdk_sha256':sha(dart),'quiet_window_authorized':True,'contract':'First measured fresh process reported separately, then3alternating repeat fresh-process pairs. Filesystem/page cache is uncontrolled and warmed by prior correctness; this is not cold-disk or persistent-server throughput. Analyzer valid wholeFileExplorer exactmachineJSON oracle. Formatter426SDKlibrary files jointlyaccepted/exactbytes proved beforehand. Wallseconds plus waited-child kernel CPU accounted. No source/file writes.','input_hashes':{str(p):sha(p) for p in [*files,*project.rglob('*.dart')] if '.dart_tool' not in p.parts},'runs':[],'complete':False}
env=dict(os.environ,TMPDIR=str(root/'target/tmp-p8-runtime'),DARTFORGE_SDK_LIB=str(dart.parent.parent/'lib'))
ledger['environment']={k:v for k,v in env.items() if k.startswith(('DARTFORGE_','DART_','PUB_')) or k=='TMPDIR'}
def save():(out/'validation.json').write_text(json.dumps(ledger,indent=2)+'\n')
for workload in ['analyze','format']:
 for round in range(4):
  order=['native','sdk'] if round%2==0 else ['sdk','native']
  outputs={}
  for variant in order:
   tool=native if variant=='native' else dart
   command=[tool,'analyze','--format=json',project] if workload=='analyze' else [tool,'format','-o','none','--summary=none','--show=none',*files]
   before=resource.getrusage(resource.RUSAGE_CHILDREN);start=time.perf_counter();result=subprocess.run(list(map(str,command)),env=env,capture_output=True,timeout=600);elapsed=time.perf_counter()-start;after=resource.getrusage(resource.RUSAGE_CHILDREN)
   label=f'{workload}-{round}-{variant}';(out/(label+'.stdout')).write_bytes(result.stdout);(out/(label+'.stderr')).write_bytes(result.stderr)
   row={'workload':workload,'round':round,'first_fresh_process':round==0,'variant':variant,'command':list(map(str,command)),'exit':result.returncode,'wall_seconds':elapsed,'waited_child_user_seconds':after.ru_utime-before.ru_utime,'waited_child_system_seconds':after.ru_stime-before.ru_stime,'stdout_sha256':hashlib.sha256(result.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(result.stderr).hexdigest()};ledger['runs'].append(row);save()
   assert result.returncode==0 and not result.stderr,(label,result.stderr.decode());outputs[variant]=result.stdout;print(label,'terminal0',flush=True)
  assert outputs['native']==outputs['sdk'],workload+' differentstdout'
ledger['summaries']={}
for workload in ['analyze','format']:
 medians={variant:statistics.median(r['wall_seconds'] for r in ledger['runs'] if r['workload']==workload and r['variant']==variant and r['round']>0) for variant in ['native','sdk']}
 ledger['summaries'][workload]={'repeat_fresh_process_wall_medians_seconds':medians,'native_over_sdk_ratio':medians['native']/medians['sdk']}
for round in range(3):
 command=['python3',root/'bench/results/2026-10-14-current-tools-status/lsp_latency.py','--quiet-window-authorized','--round',str(round)]
 result=subprocess.run(list(map(str,command)),env=env,capture_output=True,timeout=600);(out/f'lsp-{round}.stdout').write_bytes(result.stdout);(out/f'lsp-{round}.stderr').write_bytes(result.stderr);assert result.returncode==0,result.stderr.decode();print('lsp',round,'terminal0',flush=True)
rows=[]
for round in range(3):
 v=json.loads((root/f'bench/results/2026-10-14-current-tools-status/lsp-latency/round-{round}/validation.json').read_text());assert v['passed']==v['compared']==17;rows.extend(v['request_timings'])
ledger['lsp_scope']='3fresh dual-server sessions,17semanticallymatched requests each, alternatingserverorder; clientrequest roundtrip includes transport+JSON; initialmethods may waitonprojectanalysis; initialization/diagnosticwait excluded. No persistentlong-project throughput claim.'
ledger['lsp_request_rows']=rows
ledger['lsp_method_medians_seconds']={method:{variant:statistics.median(row['client_seconds'] for row in rows if row['method']==method and row['server']==variant) for variant in ['native','sdk']} for method in sorted({r['method'] for r in rows})}
ledger['inputs_unchanged']=all(sha(Path(p))==digest for p,digest in ledger['input_hashes'].items());ledger['compiler_unchanged']=sha(native)==ledger['compiler_sha256'];ledger['complete']=True;save();assert ledger['inputs_unchanged'] and ledger['compiler_unchanged']

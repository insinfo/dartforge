"""Fixed-work native ARC wall, kernel, and physical RSS comparisons."""
from pathlib import Path
import argparse,datetime,hashlib,json,os,re,statistics,subprocess,time
parser=argparse.ArgumentParser()
parser.add_argument('--output-dir',type=Path)
parser.add_argument('--skip-retain',action='store_true')
args=parser.parse_args()
root=Path.cwd();out=(args.output_dir or root/'bench/results/2026-10-14-arc-candidate-performance').resolve()
a=json.loads((out/'prepared.json').read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert a['complete'] and a['compiler_stable'] and a['source_stable']
assert all(sha(v['path'])==v['sha256'] for v in a['artifacts'].values())
assert sha(a['source'])==a['source_sha256']
commands={k:(v['path'],False) for k,v in a['artifacts'].items()}
if not args.skip_retain:
 for mode in ['legacy-arc','prepared-arc']:commands['new-retain-'+mode]=(a['artifacts']['new-'+mode]['path'],True)
env={k:v for k,v in os.environ.items() if not k.startswith('DARTFORGE_')};env['DARTFORGE_ARC_CHECK']='0'
rows=[];oracle=None;started=datetime.datetime.now(datetime.timezone.utc).isoformat()
for repetition in range(3):
 order=list(commands)
 if repetition%2:order.reverse()
 for mode in order:
  path,retain=commands[mode];run_env=dict(env)
  if retain:run_env['DARTFORGE_ARC_RETAIN_DEAD_CANDIDATES']='1'
  start=time.perf_counter_ns();p=subprocess.run(['/usr/bin/time','-l',path],env=run_env,capture_output=True,timeout=180);wall=time.perf_counter_ns()-start
  label=f'{repetition+1}-{mode}';(out/(label+'.stdout')).write_bytes(p.stdout);(out/(label+'.stderr')).write_bytes(p.stderr)
  assert p.returncode==0,(label,p.stderr)
  kernels={};result={}
  for line in p.stdout.decode().splitlines():
   m=re.fullmatch(r'(\w+): ([0-9 ]+) ms \| (.+)',line);assert m,line
   samples=list(map(int,m[2].split()));assert len(samples)==4
   kernels[m[1]]={'samples_ms':samples,'median_ms':statistics.median(samples[1:])};result[m[1]]=m[3]
  assert len(kernels)==5
  if oracle is None:oracle=result
  assert oracle==result,label
  rss=re.search(rb'(\d+)\s+maximum resident set size',p.stderr);assert rss,label
  rows.append({'mode':mode,'repeat':repetition+1,'wall_ns':wall,'rss_bytes':int(rss[1]),'kernels':kernels,'results':result,'retain_dead_candidates':retain})
  (out/'measurement.json').write_text(json.dumps({'rows':rows,'complete':False},indent=2)+'\n')
  print(label,'exact output',flush=True)
summary={}
for mode in commands:
 rr=[r for r in rows if r['mode']==mode];summary[mode]={'wall_ms':statistics.median(r['wall_ns'] for r in rr)/1e6,'rss_bytes':statistics.median(r['rss_bytes'] for r in rr),'kernels_ms':{k:statistics.median(r['kernels'][k]['median_ms'] for r in rr) for k in oracle}}
report={'rows':rows,'summary':summary,'complete':True,'started_utc':started,'finished_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'all_outputs_exact':True,'source_stable':sha(a['source'])==a['source_sha256'],'old_metadata_bytes':a['old_metadata_bytes'],'new_metadata_bytes':a['new_metadata_bytes'],'scope':'Three alternating fresh-process orders, four calls/kernel discard first, median calls then processes. /usr/bin/time macOS physical peakRSS. Producer hashes, flags and metadata sizes are recorded in prepared.json; retaining variants omitted when --skip-retain. Instrumentation disabled; new compiler platform specialization disabled for these artifacts. Millisecond kernel timer resolution; no whole-program parity claim.'}
(out/'measurement.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(summary,indent=2))

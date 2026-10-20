from pathlib import Path
import os,subprocess,time,statistics,json,hashlib
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');base=root/'target/sdk-table-batches-validation';env=os.environ.copy();env.pop('DARTFORGE_GC_STRESS',None);rows=[]
for cycle in range(30):
 for label in ['before','after','after','before']:
  exe=base/(label+'-production');start=time.perf_counter_ns();p=subprocess.run([str(exe)],env=env,capture_output=True);elapsed=(time.perf_counter_ns()-start)/1e6
  assert p.returncode==0 and p.stdout==b'parent=45\nchild=45\n' and not p.stderr,(label,p)
  rows.append(dict(cycle=cycle,warmup=cycle<5,label=label,milliseconds=elapsed))
stats={}
for label in ['before','after']:
 values=[r['milliseconds'] for r in rows if r['label']==label and not r['warmup']];stats[label]=dict(samples=len(values),median_ms=statistics.median(values),mean_ms=statistics.mean(values),minimum_ms=min(values),maximum_ms=max(values),executable_sha256=hashlib.sha256((base/(label+'-production')).read_bytes()).hexdigest())
record=dict(design='five warmup ABBA cycles; twenty-five measured ABBA cycles; whole-process wall time, including two isolates, loader and pipe capture',rows=rows,statistics=stats,median_change_percent=(stats['after']['median_ms']/stats['before']['median_ms']-1)*100)
Path('/tmp/forge-sdk-table-batches/runtime/process-measurement.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps({k:v for k,v in record.items() if k!='rows'},indent=2))

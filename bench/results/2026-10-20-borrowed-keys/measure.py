from pathlib import Path
import subprocess,json,re,time,statistics,hashlib
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-borrowed-keys');source=root/'target/read-only-allocation-validation/sdk-macos-arm64-after-inventory.json';exe=root/'target/test-tools/stream-verify-bench';rows=[]
def run(mode,measured):
 start=time.monotonic();p=subprocess.run(['/usr/bin/time','-l',str(out/(('before' if mode=='old' else 'after')+'-bench')),'new',str(source)],capture_output=True,text=True);assert p.returncode==0,(mode,p.stdout,p.stderr)
 rss=re.search(r'(\d+)\s+maximum resident set size',p.stderr);assert rss,p.stderr
 row=dict(mode=mode,measured=measured,wall_seconds=time.monotonic()-start,stage_seconds=float(p.stdout.split()[-1]),max_rss_bytes=int(rss.group(1)),exit=p.returncode,stdout=p.stdout,stderr=p.stderr);rows.append(row)
 (out/'measurement.json').write_text(json.dumps(dict(platform='macOS ARM64; time -l RSS in bytes',input=str(source.relative_to(root)),input_bytes=source.stat().st_size,input_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),runs=rows),indent=2)+'\n')
for mode in ['old','new']:run(mode,False)
for cycle in range(3):
 for mode in ['old','new','new','old']:run(mode,True)
summary={}
for mode in ['old','new']:
 selected=[r for r in rows if r['measured'] and r['mode']==mode];summary[mode]=dict(samples=len(selected),median_rss_bytes=statistics.median(r['max_rss_bytes'] for r in selected),median_stage_seconds=statistics.median(r['stage_seconds'] for r in selected))
(out/'measurement-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary),flush=True)

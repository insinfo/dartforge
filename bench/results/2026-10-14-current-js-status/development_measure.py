"""Execute only after a root-authorized isolated CPU window."""
from pathlib import Path
import datetime,hashlib,json,re,statistics,subprocess,time
ROOT=Path.cwd();OUT=ROOT/'target/current-js-status';NODE='/Applications/ChatGPT.app/Contents/Resources/cua_node/bin/node'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
ledger=json.loads((OUT/'development.json').read_text());prod=json.loads((OUT/'correctness.json').read_text());runs=[]
assert sha(ledger['compiler'])==ledger['compiler_sha256'] and sha(ledger['runtime_path'])==ledger['runtime_sha256']
start=datetime.datetime.now(datetime.timezone.utc).isoformat()
for case in ['typed_data','calls']:
    expected=prod['cases'][case]['runs']['O2']['stdout'].splitlines()
    for rep in range(3):
        for profile in ['ForgeDev','DDC'] if rep%2==0 else ['DDC','ForgeDev']:
            item=ledger['cases']['timing.'+case][profile];assert item['exit']==0
            assert all(sha(Path(item['folder'])/p)==h for p,h in item['artifacts'].items())
            command=[NODE,item['host']]
            t=time.monotonic();r=subprocess.run(command,capture_output=True,text=True,timeout=120);wall=time.monotonic()-t
            (OUT/f'development.{case}.{profile}.rep{rep}.stdout').write_text(r.stdout);(OUT/f'development.{case}.{profile}.rep{rep}.stderr').write_text(r.stderr)
            assert r.returncode==0,(profile,r.stderr)
            normalized=[];samples={}
            for line in r.stdout.splitlines():
                match=re.fullmatch(r'(\w+): ([\d ]+) us \| (.*)',line)
                if match:samples[match[1]]=list(map(int,match[2].split()));normalized.append(match[1]+': '+match[3])
                else:normalized.append(line)
            assert normalized==expected,(profile,normalized,expected)
            assert all(len(v)==6 for v in samples.values())
            runs.append({'case':case,'profile':profile,'rep':rep,'command':command,'exit':r.returncode,'wall_seconds_including_startup_and_workload':wall,'samples_us':samples,'medians_us':{k:statistics.median(v[1:]) for k,v in samples.items()}})
            print(case,profile,rep,round(wall,3),flush=True)
summary={}
for case in ['typed_data','calls']:
    rs=[r for r in runs if r['case']==case]
    summary[case]={p:{k:statistics.median(r['medians_us'][k] for r in rs if r['profile']==p) for k in rs[0]['samples_us']} for p in ['ForgeDev','DDC']}
result={'window_start_utc':start,'window_end_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'compiler_sha256':ledger['compiler_sha256'],'runtime_sha256':ledger['runtime_sha256'],'timer_contract':'Original six-call kernel contract; performance.now exposed uniformly via self/window; median five after first warmup, median three fresh process reps.','scope':'Two development kernels, not broad parity. Process wall includes startup plus all original work and is not pure startup latency.','runs':runs,'summary_medians_us':summary}
(OUT/'development-performance.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(summary));print('terminal',flush=True)

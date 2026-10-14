"""Run only after root grants an isolated CPU window."""
from pathlib import Path
import datetime, hashlib,json,re,statistics,subprocess,time
ROOT=Path.cwd();OUT=ROOT/'target/current-js-status';NODE='/Applications/ChatGPT.app/Contents/Resources/cua_node/bin/node'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
ledger=json.loads((OUT/'artifacts.json').read_text()); correct=json.loads((OUT/'correctness.json').read_text())
assert all(sha(p)==h for p,h in ledger['inputs'].items())
assert sha(ledger['compiler'])==ledger['compiler_sha256']
start=datetime.datetime.now(datetime.timezone.utc).isoformat();runs=[]
for case in ['json','typed_data','objects_in_collections','calls']:
    if not correct['cases'][case]['Forge_JS_exact_O2']:
        print('Not timed: unmatched/rejected',case,flush=True);continue
    expected=correct['cases'][case]['runs']['O2']['stdout'].splitlines()
    for rep in range(3):
        profiles=['raw','min','O2'] if rep%2==0 else ['O2','min','raw']
        for profile in profiles:
            item=ledger['artifacts']['timing.'+case][profile]
            assert item['exit']==0 and sha(item['artifact'])==item['sha256']
            cmd=[NODE,'-e','globalThis.self=globalThis;globalThis.window=globalThis;require(process.argv[1])',item['artifact']]
            t=time.monotonic();r=subprocess.run(cmd,capture_output=True,text=True,timeout=120);wall=time.monotonic()-t
            (OUT/f'{case}.{profile}.rep{rep}.stdout').write_text(r.stdout);(OUT/f'{case}.{profile}.rep{rep}.stderr').write_text(r.stderr)
            assert r.returncode==0 and not r.stderr,(case,profile,r.stderr)
            normalized=[];samples={}
            for line in r.stdout.splitlines():
                match=re.fullmatch(r'(\w+): ([\d ]+) us \| (.*)',line)
                if match:
                    values=list(map(int,match[2].split()));assert len(values)==(4 if match[1] in ['decode_large','encode_large'] else 6)
                    samples[match[1]]=values;normalized.append(match[1]+': '+match[3])
                else:normalized.append(line)
            assert normalized==expected,(case,profile,normalized,expected)
            runs.append({'case':case,'profile':profile,'rep':rep,'command':cmd,'exit':r.returncode,'wall_seconds':wall,'samples_us':samples,'medians_us':{k:statistics.median(v[1:]) for k,v in samples.items()}})
            print(case,profile,rep,round(wall,3),flush=True)
summary={}
for case in {r['case'] for r in runs}:
    grouped=[r for r in runs if r['case']==case]
    summary[case]={p:{k:statistics.median(r['medians_us'][k] for r in grouped if r['profile']==p) for k in grouped[0]['samples_us']} for p in ['raw','min','O2']}
assert all(sha(p)==h for p,h in ledger['inputs'].items())
result={'window_start_utc':start,'window_end_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'node_version':subprocess.check_output([NODE,'--version'],text=True).strip(),'node_binary_sha256':sha(NODE),'compiler_sha256':ledger['compiler_sha256'],'timer_contract':'Uniform self/window expose performance.now to SDK initTicker; floor microsecond precision. Short samples have greater quantization error.','execution_contract':'Fresh Node per profile and repetition; original six calls (four large JSON), discard first warmup call, median remaining then median three processes. Alternating profile order. No broad parity claim.','runs':runs,'summary_medians_us':summary}
(OUT/'performance.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(summary));print('terminal quiet window can be released',flush=True)

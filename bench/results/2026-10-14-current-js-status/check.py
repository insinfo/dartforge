"""Use one-call untimed snapshots to establish exact JavaScript result oracles."""
from pathlib import Path
import hashlib,json,subprocess
ROOT=Path.cwd(); OUT=ROOT/'target/current-js-status'
NODE='/Applications/ChatGPT.app/Contents/Resources/cua_node/bin/node'
DART='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
ledger=json.loads((OUT/'artifacts.json').read_text()); report={'cases':{},'scope':'Untimed one-call kernel snapshots. JSON VM generated large integers may differ from JS exact integer policy; JS oracle is checked O2.'}
for case in ['json','typed_data','objects_in_collections','calls']:
    profiles=ledger['artifacts']['correctness.'+case];runs={}
    for profile in ['VM','raw','min','O2']:
        if profile!='VM' and profiles[profile]['exit']!=0:
            runs[profile]={'compile_exit':profiles[profile]['exit']}; continue
        command=[DART,'run',str(OUT/'correctness'/f'{case}.dart')] if profile=='VM' else [NODE,'-e','globalThis.self=globalThis;globalThis.window=globalThis;require(process.argv[1])',profiles[profile]['artifact']]
        r=subprocess.run(command,capture_output=True,text=True,timeout=120)
        (OUT/f'check.{case}.{profile}.stdout').write_text(r.stdout);(OUT/f'check.{case}.{profile}.stderr').write_text(r.stderr)
        runs[profile]={'command':command,'exit':r.returncode,'stdout':r.stdout,'stderr':r.stderr}
    matched=all(runs[p].get('exit')==0 and runs[p]['stdout']==runs['O2'].get('stdout') for p in ['raw','min','O2'])
    report['cases'][case]={'runs':runs,'Forge_JS_exact_O2':matched,'VM_exact_O2':runs['VM']['stdout']==runs['O2'].get('stdout')}
    print(case,'exact O2',matched,'VM exact',report['cases'][case]['VM_exact_O2'],flush=True)
assert all(sha(p)==h for p,h in ledger['inputs'].items())
(OUT/'correctness.json').write_text(json.dumps(report,indent=2)+'\n')

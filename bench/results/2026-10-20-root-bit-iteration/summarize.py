from pathlib import Path
import json,statistics,re
p=Path('/tmp/forge-root-bit-iteration/benchmark')
rows=json.loads((p/'runs.json').read_text())
assert len(rows)==10 and json.loads((p/'state.json').read_text())['status']=='finished'
summary=[]
for variant in ['before','after']:
 selected=[r for r in rows if r['variant']==variant and not r['warmup']]
 assert len(selected)==4 and all(r['exit']==0 for r in selected)
 for r in selected:
  content=(p/(r['label']+'.log')).read_text()
  r['llvm_ir_seconds']=float(re.search(r'LLVM IR: +([0-9.]+)s',content)[1])
 summary.append(dict(variant=variant,samples=len(selected),**{key:dict(median=statistics.median(r[key] for r in selected),min=min(r[key] for r in selected),max=max(r[key] for r in selected)) for key in ['wall_seconds','user_seconds','system_seconds','max_rss_bytes','llvm_ir_seconds']}))
assert len({r['ir_sha256'] for r in rows})==1
(p/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary,indent=2))

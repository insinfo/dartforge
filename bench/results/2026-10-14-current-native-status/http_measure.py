"""Run fixed-work HTTP comparisons with frozen current artifacts."""
from pathlib import Path
import datetime
import hashlib
import json
import os
import subprocess
root=Path.cwd()
out=root/'bench/results/2026-10-14-current-native-status'
a=json.loads((out/'prepared.json').read_text())
sha=lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
artifacts=[a['artifacts'][m+'/server_utf8'] for m in ['tracing','official']]
assert all(sha(v['path'])==v['sha256'] for v in artifacts)
assert all(sha(p)==h for p,h in a['inputs'].items())
command=['python3','scripts/measure-http-pairs.py',*[v['path'] for v in artifacts], '--requests','20000','--warmup','1000','--rounds','3','--routes','/,/json','--output',str(out/'http.json')]
started=datetime.datetime.now(datetime.timezone.utc).isoformat()
p=subprocess.run(command,env={k:v for k,v in os.environ.items() if not k.startswith('DARTFORGE_')},capture_output=True,timeout=600)
(out/'http.stdout').write_bytes(p.stdout)
(out/'http.stderr').write_bytes(p.stderr)
(out/'http-provenance.json').write_text(json.dumps({'command':command,'started_utc':started,'finished_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'exit':p.returncode,'artifacts':artifacts,'harness_sha256':sha(root/'scripts/measure-http-pairs.py'),'inputs_stable':all(sha(p)==h for p,h in a['inputs'].items()),'scope':'Fixed work, one local Python keep-alive connection, verified response bytes, CPU ps counter resolution0.01s; throughput client-limited.'},indent=2)+'\n')
assert p.returncode==0,p.stderr.decode()
print(p.stdout.decode())

set -euo pipefail
python3 - <<'PYCODE'
from pathlib import Path
import os,subprocess,json,hashlib
shared=Path('/media/psf/ssd_dock/MyRustProjects/dartforge/target/workspace-format-validation')
builds=json.loads((shared/'cross-builds.json').read_text())
record=next(r for r in builds if r['target']=='linux-arm64');assert record['exit']==0
source=shared/'cross-linux-arm64';data=source.read_bytes();assert hashlib.sha256(data).hexdigest()==record['sha256']
binary=Path('/tmp')/('dartforge-workspace-format-'+record['sha256']);binary.write_bytes(data);binary.chmod(0o755)
assert hashlib.sha256(binary.read_bytes()).hexdigest()==record['sha256']
rows=[]
for stress in [False,True]:
 env=os.environ.copy();env.pop('DARTFORGE_GC_STRESS',None)
 if stress:env['DARTFORGE_GC_STRESS']='1'
 p=subprocess.run([str(binary)],env=env,capture_output=True,text=True,timeout=60)
 rows.append(dict(stress=stress,exit=p.returncode,stdout=p.stdout,stderr=p.stderr,sha256=record['sha256'],uid=os.getuid()))
 (shared/'linux-runtime.json').write_text(json.dumps(rows,indent=2)+'\n')
 assert p.returncode==0 and p.stdout=='parent=45\nchild=45\n' and not p.stderr,rows[-1]
print(json.dumps(rows,indent=2))
PYCODE
cp /tmp/forge-workspace-format-linux-build-final.log /media/psf/ssd_dock/MyRustProjects/dartforge/target/workspace-format-validation/linux-build.log

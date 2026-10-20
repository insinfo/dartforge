from pathlib import Path
import json,hashlib,re
out=Path('/Volumes/ssd_dock/MyRustProjects/dartforge/target/current-desktop-validation')
def read(path):return json.loads((out/path).read_text(encoding='utf-8-sig'))
assert read('ui-inputs-host.json')==read('ui-inputs-linux.json')==read('ui-inputs-windows.json')
for row in read('builds.json'):assert row['exit']==0
for row in read('builds.json')[1:]:
 target=out/(row['name']+('.exe' if row['name'].startswith('windows') else ''));assert hashlib.sha256(target.read_bytes()).hexdigest()==row['executable_sha256']
for kind,expected in [('aot',2),('jit',2)]:
 r=read('linux-ui-'+kind+'-report.json');assert r['state']['status']=='finished' and len(r['runs'])==expected
 for row in r['runs']:
  assert row['exit']==0 and row['stderr']==''
  if row['name'].startswith('context'):assert 'CONTEXT_RETARGET=PASS' in row['stdout']
r=read('windows-ui-report.json');assert r['state']['status']=='finished' and len(r['runs'])==4
for row in r['runs']:
 assert row['exit']==0 and row['stderr']==''
 if row['name'].startswith('context'):assert 'CONTEXT_RETARGET=PASS' in row['stdout']
for platform in ['linux','windows']:
 rows=read(platform+'-sdk-modes.json');assert len(rows)==3
 for r in rows:assert r['exit']==0 and r['stdout'].replace('\r\n','\n')=='["--sdk","application-owned","--"]\n'
for report in ['macos-ui/results-all.json','macos-jit-fixed/results-jit.json']:
 r=read(report)
 for row in r['runs']:
  if report.startswith('macos-ui/') and '-jit-' in row['name']:continue
  assert row['exit']==0 and row['passed']
assert read('sdk-red.json')['exit']==101 and read('sdk-green.json')['exit']==0
assert len(read('final-checks.json'))==5 and all(r['exit']==0 for r in read('final-checks.json'))
print('Matrix, SDK selection, source identity and required checks verified')

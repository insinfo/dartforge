from pathlib import Path
import json,hashlib,shutil
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-borrowed-keys/runtime');art=root/'target/borrowed-keys-validation';rows=[]
for name in ['production','development','unpruned']:
 a=(art/'.df_tmp'/f'before-{name}.ll').read_bytes();b=(art/'.df_tmp'/f'after-{name}.ll').read_bytes();assert a==b,name;rows.append(dict(name=name,identical=True,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()))
(out/'ir-comparison.json').write_text(json.dumps(rows,indent=2)+'\n')
baseline=root/'target/stream-verify-validation/sdk-macos-arm64-after-inventory.json';before=json.loads(baseline.read_bytes())
for path in sorted((root/'target/rpo-native-cache/sdk').glob('*/allocation-inventory.json'),key=lambda p:p.stat().st_mtime,reverse=True):
 after=json.loads(path.read_bytes())
 if after['production'] and after['compiler']!=before['compiler']:break
else:raise RuntimeError('missing new production SDK inventory')
assert before['inventory']==after['inventory']
shutil.copy2(path,art/'sdk-macos-arm64-after-inventory.json')
(out/'sdk-comparison.json').write_text(json.dumps(dict(baseline=str(baseline.relative_to(root)),source=str(path.relative_to(root)),compiler=after['compiler'],payload_identical=True,functions=len(after['inventory']['functions'])),indent=2)+'\n')
print('Three host IRs and production SDK inventory payload are unchanged.')

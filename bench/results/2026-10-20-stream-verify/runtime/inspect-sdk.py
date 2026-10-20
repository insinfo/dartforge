from pathlib import Path
import json,shutil
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-stream-verify/runtime');art=root/'target/stream-verify-validation';rows=[];compiler=None
for target,prefix in [('macos-arm64',''),('linux-arm64','aarch64-unknown-linux-gnu'),('windows-x64','x86_64-pc-windows-msvc')]:
 before_path=root/f'target/inventory-io-validation/sdk-{target}-after-inventory.json';before=json.loads(before_path.read_bytes())
 for path in sorted((root/'target/rpo-native-cache'/prefix/'sdk').glob('*/allocation-inventory.json'),key=lambda p:p.stat().st_mtime,reverse=True):
  after=json.loads(path.read_bytes())
  if after['production'] and after['compiler']!=before['compiler'] and (compiler is None or after['compiler']==compiler):break
 else:raise RuntimeError('missing final inventory '+target)
 if compiler is None:compiler=after['compiler']
 assert before['inventory']==after['inventory'],target
 shutil.copy2(path,art/f'sdk-{target}-after-inventory.json');shutil.copy2(before_path,art/f'sdk-{target}-before-inventory.json')
 rows.append(dict(target=target,compiler=after['compiler'],source=str(path.relative_to(root)),inventory_payload_identical=True,functions=len(after['inventory']['functions']),known=sum(f['transitive_classes'] is not None for f in after['inventory']['functions'])));print(target,'inventory payload identical',flush=True)
(out/'sdk-comparison.json').write_text(json.dumps(rows,indent=2)+'\n')

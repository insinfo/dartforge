from pathlib import Path
import json,shutil
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');art=root/'target/read-only-allocation-validation';out=Path('/tmp/forge-read-only-allocation/runtime');report=[];compiler=None
for target,prefix in [('macos-arm64',''),('linux-arm64','aarch64-unknown-linux-gnu'),('windows-x64','x86_64-pc-windows-msvc')]:
 before=json.loads((art/f'sdk-{target}-before-inventory.json').read_bytes());old={f['key']:f for f in before['inventory']['functions']}
 for path in sorted((root/'target/rpo-native-cache'/prefix/'sdk').glob('*/allocation-inventory.json'),key=lambda p:p.stat().st_mtime,reverse=True):
  after=json.loads(path.read_bytes())
  if after['production'] and after['compiler']!=before['compiler'] and (compiler is None or after['compiler']==compiler):break
 else:raise RuntimeError('missing final inventory '+target)
 if compiler is None:compiler=after['compiler']
 functions=after['inventory']['functions'];exposed={f['key'] for f in functions if f['exposure_obligations']}
 while True:
  size=len(exposed);exposed.update(f['key'] for f in functions if any(d in exposed for d in f['dependencies']))
  if len(exposed)==size:break
 gained=[f for f in functions if f['key'] in old and old[f['key']]['transitive_classes'] is None and f['transitive_classes'] is not None]
 lost=[f for f in functions if f['key'] in old and old[f['key']]['transitive_classes'] is not None and f['transitive_classes'] is None]
 assert all(f['key'] in exposed for f in gained),'new knowledge without exposure needs review'
 assert all(f['body_digest']==old[f['key']]['body_digest'] for f in functions if f['key'] in old)
 shutil.copy2(path,art/f'sdk-{target}-after-inventory.json')
 row=dict(target=target,compiler=after['compiler'],source=str(path.relative_to(root)),functions=len(functions),before_known=sum(f['transitive_classes'] is not None for f in old.values()),after_known=sum(f['transitive_classes'] is not None for f in functions),gained=[dict(key=f['key'],classes=f['transitive_classes'],own_exposure=f['exposure_obligations']) for f in gained],lost=[dict(key=f['key'],obligations=f['unknown_obligations']) for f in lost],all_bodies_unchanged=True,all_gained_have_transitive_exposure=True)
 report.append(row);print(target,row['before_known'],row['after_known'],'gained',len(gained),'lost',len(lost),flush=True)
(out/'sdk-comparison.json').write_text(json.dumps(report,indent=2)+'\n')

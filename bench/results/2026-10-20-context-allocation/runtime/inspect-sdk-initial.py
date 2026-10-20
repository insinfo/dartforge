from pathlib import Path
import json,shutil
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');dest=root/'target/context-allocation-validation';out=Path('/tmp/forge-context-allocation/runtime')
base=json.loads((dest/'sdk-before-inventory.json').read_bytes());pre=json.loads((dest/'sdk-pre-independent-inventory.json').read_bytes());report=[];final_compiler=None
for target,prefix in [('macos-arm64',''),('linux-arm64','aarch64-unknown-linux-gnu'),('windows-x64','x86_64-pc-windows-msvc')]:
 cache=root/'target/rpo-native-cache'/prefix/'sdk';files=sorted(cache.glob('*/allocation-inventory.json'),key=lambda p:p.stat().st_mtime,reverse=True)
 after=None;before=base if target=='macos-arm64' else None
 for path in files:
  data=json.loads(path.read_bytes())
  if not data['production']:continue
  if data['compiler']==base['compiler'] and before is None:before=data
  if data['compiler'] not in [base['compiler'],pre['compiler']] and (final_compiler is None or data['compiler']==final_compiler) and after is None:
   after=data;after_path=path
 assert after is not None,(target,'final inventory missing')
 if target=='macos-arm64':
  final_compiler=after['compiler'];assert after['inventory']==pre['inventory'],'independent review changes current inventory semantics'
 assert before is not None,(target,'baseline inventory missing')
 contexts=[f for f in after['inventory']['functions'] if f['symbol']=='df.context'];assert len(contexts)==15 and all(f['transitive_classes']==[] and f['unknown_obligations']==[] for f in contexts)
 if target!='macos-arm64':assert all([c['symbol'] for c in f['calls']]==['llvm.threadlocal.address.p0','dartforge_context'] for f in contexts)
 old={f['key']:f for f in before['inventory']['functions']};changed=[dict(key=f['key'],body_unchanged=f['body_digest']==old[f['key']]['body_digest'],before=old[f['key']]['transitive_classes'],after=f['transitive_classes'],exposure=f['exposure_obligations'],calls=f['calls']) for f in after['inventory']['functions'] if f['key'] in old and f['transitive_classes']!=old[f['key']]['transitive_classes']]
 assert len(changed)==15 and all(c['body_unchanged'] for c in changed)
 shutil.copy2(after_path,dest/f'sdk-{target}-after-inventory.json')
 report.append(dict(target=target,compiler=after['compiler'],source=str(after_path.relative_to(root)),functions=len(after['inventory']['functions']),before_known=sum(f['transitive_classes'] is not None for f in old.values()),after_known=sum(f['transitive_classes'] is not None for f in after['inventory']['functions']),changed=changed))
 print(target,report[-1]['before_known'],report[-1]['after_known'],flush=True)
(out/'sdk-footprint-comparison.json').write_text(json.dumps(report,indent=2)+'\n')

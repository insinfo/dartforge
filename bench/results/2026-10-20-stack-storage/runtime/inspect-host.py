from pathlib import Path
import json,hashlib,shutil,collections
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-stack-storage/runtime');art=root/'target/stack-storage-validation';rows=[]
for name in ['production','development','unpruned']:
 a=(art/'.df_tmp'/f'before-{name}.ll').read_bytes();b=(art/'.df_tmp'/f'after-{name}.ll').read_bytes();assert a==b,name;rows.append(dict(name=name,identical=True,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()))
(out/'ir-comparison.json').write_text(json.dumps(rows,indent=2)+'\n')
baseline=root/'target/stack-read-validation/sdk-macos-arm64-after-inventory.json';before=json.loads(baseline.read_bytes())
for path in sorted((root/'target/rpo-native-cache/sdk').glob('*/allocation-inventory.json'),key=lambda p:p.stat().st_mtime,reverse=True):
 after=json.loads(path.read_bytes())
 if after['production'] and after['compiler']!=before['compiler']:break
else:raise RuntimeError('missing new production SDK inventory')
a=before['inventory'];b=after['inventory'];assert a['modules']==b['modules'] and a['globals']==b['globals'] and a['scope']==b['scope']
def groups(records):
 result=collections.defaultdict(list)
 for f in records:result[(f['module'],f['symbol'],f['body_digest'],f['local'])].append(f)
 return result
old=groups(a['functions']);new=groups(b['functions']);assert old.keys()==new.keys();changed=0;resolved=0;new_known=0;new_stack_exposures=0
for key,old_records in old.items():
 new_records=new[key];assert len(old_records)==len(new_records)
 sort=lambda f:json.dumps({k:v for k,v in f.items() if k not in ['unknown_obligations','exposure_obligations','transitive_classes']},sort_keys=True)
 for x,y in zip(sorted(old_records,key=sort),sorted(new_records,key=sort)):
  assert sort(x)==sort(y),key
  removed=set(x['unknown_obligations'])-set(y['unknown_obligations']);assert removed <= {'unresolved_raw_memory_allocation'},key
  assert set(y['unknown_obligations']) <= set(x['unknown_obligations']),key
  extra=set(y['exposure_obligations'])-set(x['exposure_obligations']);assert extra <= {'raw_memory_or_pointer_operation'},key
  assert set(x['exposure_obligations']) <= set(y['exposure_obligations']),key
  if x['transitive_classes'] is not None:assert x['transitive_classes']==y['transitive_classes'],key
  elif y['transitive_classes'] is not None:new_known+=1
  changed+=x!=y;resolved+=len(removed);new_stack_exposures+=bool(extra)
shutil.copy2(path,art/'sdk-macos-arm64-after-inventory.json')
result=dict(baseline=str(baseline.relative_to(root)),source=str(path.relative_to(root)),compiler=after['compiler'],functions=len(b['functions']),changed_functions=changed,resolved_raw_memory_obligations=resolved,new_stack_exposures=new_stack_exposures,newly_known_allocations=new_known,known_before=sum(f['transitive_classes'] is not None for f in a['functions']),known_after=sum(f['transitive_classes'] is not None for f in b['functions']),modules_and_globals_identical=True)
(out/'sdk-comparison.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)

from pathlib import Path
import json,hashlib,collections,gc
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge/target/rpo-native-cache/sdk')
old=root/'44653ec348452876e237aace2cdee85c';new=root/'cf7a041bb12bd7bf7705d1114ede19c4'
def inventory(directory):
 p=directory/'allocation-inventory.json';x=json.loads(p.read_text());v=x['inventory']
 functions=collections.Counter(hashlib.sha256(json.dumps(f,sort_keys=True,separators=(',',':')).encode()).hexdigest() for f in v['functions'])
 globals={}
 for g in v['globals']:
  key=(g['module'],g['symbol'],g['initializer_digest']);assert key not in globals
  globals[key]=g
 result=dict(modules=v['modules'],scope=v['scope'],functions=functions,globals=globals,known=sum(f['transitive_classes'] is not None for f in v['functions']),sha256=hashlib.sha256(p.read_bytes()).hexdigest())
 del x,v;gc.collect();return result
def summary(path):
 lines=path.read_text().splitlines();names=[l[2:] for l in lines if l.startswith('N\t')];definitions={};records=[]
 for line in lines[1:]:
  fields=line.split('\t');tag=fields[0]
  if tag=='N':continue
  if tag=='D':definitions[names[int(fields[1])]]=(tuple(sorted(names[int(i)] for i in fields[2].split())),fields[3]);continue
  if tag=='T':fields[1]=names[int(fields[1])];fields[3]=tuple((h,names[int(node)]) for h,node in (p.split(':') for p in fields[3].split()))
  elif tag in ['I','X','R','C','E','F','V']:fields[1]=names[int(fields[1])]
  elif tag=='B':fields[1]=names[int(fields[1])];fields[2]=names[int(fields[2])]
  else:assert tag in ['S','L','U','K','A','P'],tag
  records.append(repr(fields))
 return dict(names=set(names),definitions=definitions,records=collections.Counter(records))
a=inventory(old);b=inventory(new);assert a['modules']==b['modules'];assert a['scope']==b['scope'];assert a['functions']==b['functions'];assert a['known']==b['known'];assert set(a['globals'])==set(b['globals']);global_changes=[]
for key,g in a['globals'].items():
 other=b['globals'][key];before={k:v for k,v in g.items() if k!='references'};after={k:v for k,v in other.items() if k!='references'};assert before==after
 if g['references']!=other['references']:global_changes.append(dict(module=key[0],symbol=key[1],before=g['references'],after=other['references']))
changes=[];bindings=[]
for p in sorted(old.glob('*.poda')):
 q=new/p.name;x=summary(p);y=summary(q);assert x['records']==y['records'],p;assert set(x['definitions'])==set(y['definitions']);assert y['names']<=x['names']
 for name,(refs,calls) in x['definitions'].items():
  other_refs,other_calls=y['definitions'][name];assert calls==other_calls;assert set(other_refs)<=set(refs)
  if refs!=other_refs:changes.append(dict(module=p.stem,definition=name,removed=sorted(set(refs)-set(other_refs)),remaining=list(other_refs)))
 bindings.append(dict(module=p.stem,before=str(p),after=str(q),before_sha256=hashlib.sha256(p.read_bytes()).hexdigest(),after_sha256=hashlib.sha256(q.read_bytes()).hexdigest(),removed_names=sorted(x['names']-y['names'])))
result=dict(before=str(old),after=str(new),before_inventory_sha256=a['sha256'],after_inventory_sha256=b['sha256'],module_ir_identical=True,all_function_records_identical=True,functions=sum(a['functions'].values()),known_allocation_closures=a['known'],changed_globals=global_changes,changed_summary_definitions=changes,summaries=bindings)
Path('/tmp/forge-byte-literal-pruning/sdk-comparison.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:result[k] for k in ['module_ir_identical','all_function_records_identical','functions','known_allocation_closures']},indent=2));print('Changed globals',len(global_changes),'changed summary definitions',len(changes))

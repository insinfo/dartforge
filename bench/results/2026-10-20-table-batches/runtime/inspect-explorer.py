from pathlib import Path
import re,json,itertools,hashlib
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');directory=root/'target/batch-tables-validation';out=Path('/tmp/forge-batch-tables-runtime');before=directory/'before.ll';after=directory/'after.ll';calls=[];getters=[];function=''
for line in before.open():
 if line.startswith('define '):function=line.split('@',1)[1].split('(',1)[0]
 if function=='df.register.program' and 'call void @dartforge_register_table(' in line:
  m=re.search(r'@dartforge_register_table\(i64 (\d+), ptr @([^ )]+)\)',line);assert m,line;calls.append((int(m[1]),m[2],line))
arrays=[]
for line in after.open():
 if line.startswith('@df.table_batch.df.register.program = '):arrays.append(line);getters=re.findall(r'ptr @([^,\] ]+)',line)
assert len(arrays)==1 and len(calls)==3758 and getters==[symbol for cid,symbol,line in calls]
expected={symbol:cid for cid,symbol,line in calls};headers={};returns={};function=None;body=[]
for line in before.open():
 if line.startswith('@df.mt.') and '$d = ' in line and '} {' in line:
  symbol=line[1:].split(' = ',1)[0];values=line.split('} {',1)[1].lstrip();match=re.match(r'i64 ([-0-9]+),',values)
  if match:headers[symbol]=int(match[1])
 if line.startswith('define '):
  candidate=line.split('@',1)[1].split('(',1)[0];function=candidate if candidate in expected else None;body=[];continue
 if function:
  if line.strip()=='}':
   instructions=[entry.strip() for entry in body if entry.strip() and not entry.strip().endswith(':') and not entry.strip().startswith(';')]
   assert len(instructions)==1 and instructions[0].startswith('ret ptr @'),(function,instructions)
   returns[function]=instructions[0].removeprefix('ret ptr @');function=None
  else:body.append(line)
assert len(returns)==len(expected) and all(headers[returns[symbol]]==cid for symbol,cid in expected.items())
changes={'batch_arrays':0,'batch_calls':0,'batch_declarations':0}
def normalized_after():
 for line in after.open():
  if line.startswith('@df.table_batch.df.register.program = '):changes['batch_arrays']+=1;continue
  if line.startswith('declare ') and '@dartforge_register_tables(' in line:changes['batch_declarations']+=1;continue
  if 'call void @dartforge_register_tables(ptr @df.table_batch.df.register.program,' in line:
   changes['batch_calls']+=1
   yield from (line for cid,symbol,line in calls)
   continue
  yield line
mismatches=[]
for index,(left,right) in enumerate(itertools.zip_longest(before.open(),normalized_after()),1):
 if left!=right:
  mismatches.append(dict(line=index,before=left[:250] if left else None,after=right[:250] if right else None))
  if len(mismatches)==12:break
record=dict(before_bytes=before.stat().st_size,after_bytes=after.stat().st_size,ir_bytes_saved=before.stat().st_size-after.stat().st_size,ordered_getters=len(getters),pure_constant_getters=len(returns),matching_embedded_cids=len(expected),changes=changes,normalized_identical=not mismatches,mismatches=mismatches)
(out/'explorer-comparison.json').write_text(json.dumps(record,indent=2)+'\n');print(record);assert not mismatches and changes['batch_calls']==1 and changes['batch_arrays']==1

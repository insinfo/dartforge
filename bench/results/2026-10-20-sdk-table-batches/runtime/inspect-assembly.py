from pathlib import Path
import re,json,hashlib
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');folder=root/'target/sdk-table-batches-validation/.df_tmp';rows=[]
for profile in ['production','unpruned']:
 before=(folder/f'before-{profile}.ll').read_text();after=(folder/f'after-{profile}.ll').read_text()
 old=re.search(r'define void @df\.register\.sdk_tables\(\) \{\n(.*?)\n\}',before,re.S).group(1)
 calls=re.findall(r'^  call void @dartforge_register_table\(i64 (\d+), ptr @([^\s)]+)\)$',old,re.M);assert calls
 expected=[name for cid,name in calls]
 arrays=re.findall(r'^@df\.table_batch\.df\.register\.sdk_tables = private unnamed_addr constant \[(\d+) x ptr\] \[(.*?)\], align 8\n',after,re.M);assert len(arrays)==1
 count,pointers=arrays[0];actual=[p.removeprefix('ptr @') for p in pointers.split(', ')];assert int(count)==len(calls) and actual==expected
 for cid,getter in calls:
  assert re.search(r'^define ptr @'+re.escape(getter)+r'\(\) \{\nb0:\n  ret ptr @'+re.escape(getter)+r'\$d\n\}',after,re.M),getter
  assert re.search(r'^@'+re.escape(getter)+r'\$d = private unnamed_addr constant \{[^\n]+?\} \{ i64 '+cid+r', i64 \d+',after,re.M),getter
 normalized=re.sub(r'^@df\.table_batch\.df\.register\.sdk_tables = private unnamed_addr constant \[\d+ x ptr\] \[.*?\], align 8\n','',after,flags=re.M)
 scalar='\n'.join(f'  call void @dartforge_register_table(i64 {cid}, ptr @{name})' for cid,name in calls)
 normalized=normalized.replace(f'  call void @dartforge_register_tables(ptr @df.table_batch.df.register.sdk_tables, i64 {count})',scalar)
 assert normalized==before,profile
 rows.append(dict(profile=profile,ordered_getters=len(calls),matching_embedded_cids=len(calls),normalized_identical=True,before_bytes=len(before.encode()),after_bytes=len(after.encode()),saved_bytes=len(before.encode())-len(after.encode()),before_sha256=hashlib.sha256(before.encode()).hexdigest(),after_sha256=hashlib.sha256(after.encode()).hexdigest()))
Path('/tmp/forge-sdk-table-batches/runtime/assembly-comparison.json').write_text(json.dumps(rows,indent=2)+'\n');print(json.dumps(rows,indent=2))

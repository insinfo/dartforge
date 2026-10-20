from pathlib import Path
import json,hashlib
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');source=Path('/tmp/forge-pruned-jit');dest=root/'bench/results/2026-10-20-jit-workspace-checks';rows=[]
def sha(data):return hashlib.sha256(data).hexdigest()
for p in sorted(source.iterdir()):
 if not p.is_file() or p.suffix not in {'.json','.log','.py'}:continue
 original=p.read_bytes();text='\n'.join(line.rstrip() for line in original.decode('utf-8-sig').splitlines()).rstrip('\n');data=(text+'\n').encode() if text else b'';q=dest/p.name;q.write_bytes(data);rows.append(dict(source=str(p),stored=p.name,original_bytes=len(original),original_sha256=sha(original),stored_bytes=len(data),stored_sha256=sha(data)))
(dest/'log-bindings.json').write_text(json.dumps(rows,indent=2)+'\n')
paths=[root/'target/test-tools/dartforge-run-ir-pruned-check',root/'target/test-tools/dartforge-stack-storage',root/'target/debug/dartforge-run-ir',Path('/tmp/forge-string-length-reuse/development.ll'),Path('/tmp/forge-string-length-reuse/lengths.dart'),root/'target/string-length-native-cache/sdk/6b6831042293752baa4cd97534016ec3/libdfsdk_6b6831042293752b.dylib',root/'target/string-length-native-cache/sdk/6b6831042293752baa4cd97534016ec3/exportados.def'];records=[]
for p in paths:
 h=hashlib.sha256()
 with p.open('rb') as f:
  for chunk in iter(lambda:f.read(1024*1024),b''):h.update(chunk)
 records.append(dict(path=str(p.relative_to(root)) if p.is_relative_to(root) else str(p),bytes=p.stat().st_size,sha256=h.hexdigest()))
(dest/'artifact-bindings.json').write_text(json.dumps(records,indent=2)+'\n')
for row in json.loads((source/'source-bindings.json').read_text()):assert sha((root/row['path']).read_bytes())==row['sha256'],row['path']
print('Stored',len(rows),'verification records and',len(records),'artifact bindings.')

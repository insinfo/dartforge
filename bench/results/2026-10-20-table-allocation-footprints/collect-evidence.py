from pathlib import Path
import hashlib,json,shutil,subprocess
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');source=Path('/tmp/forge-table-footprints');dest=root/'bench/results/2026-10-20-table-allocation-footprints';dest.mkdir(exist_ok=True)
rows=[]
def sha(data):return hashlib.sha256(data).hexdigest()
for p in sorted(source.rglob('*')):
 if not p.is_file() or p.suffix not in {'.json','.log','.py','.txt'} or p.name=='fmt-original.log':continue
 relative=p.relative_to(source);data=p.read_bytes();original=data
 if data:
  text='\n'.join(line.rstrip() for line in data.decode().splitlines()).rstrip('\n');data=(text+'\n').encode() if text else b''
 q=dest/relative;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(data)
 rows.append(dict(source=str(p),stored=str(relative),original_sha256=sha(original),original_bytes=len(original),stored_sha256=sha(data),stored_bytes=len(data)))
(dest/'log-bindings.json').write_text(json.dumps(rows,indent=2)+'\n')
for row in json.loads((source/'source-bindings-final.json').read_text()):assert sha((root/row['path']).read_bytes())==row['sha256'],row['path']
artifacts=[]
paths=[root/'target/test-tools'/name for name in ['dartforge-before-table-footprints','dartforge-table-footprints-preliminary','dartforge-table-footprints']]
paths += [p for p in (root/'target/table-footprints-validation').glob('*') if p.is_file()]
for p in paths:
 data=p.read_bytes();artifacts.append(dict(path=str(p.relative_to(root)),bytes=len(data),sha256=sha(data)))
(dest/'artifact-bindings.json').write_text(json.dumps(artifacts,indent=2)+'\n');print('Stored',len(rows),'logs/scripts and',len(artifacts),'artifact bindings')

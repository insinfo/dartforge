from pathlib import Path
import hashlib,json
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');source=Path('/tmp/forge-sdk-table-batches');dest=root/'bench/results/2026-10-20-sdk-table-batches';dest.mkdir(exist_ok=True);rows=[]
def sha(data):return hashlib.sha256(data).hexdigest()
def copy(p,relative):
 original=p.read_bytes();text='\n'.join(line.rstrip() for line in original.decode('utf-8-sig').splitlines()).rstrip('\n');data=(text+'\n').encode() if text else b'';q=dest/relative;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(data);rows.append(dict(source=str(p),stored=str(relative),original_bytes=len(original),original_sha256=sha(original),stored_bytes=len(data),stored_sha256=sha(data)))
for p in sorted(source.rglob('*')):
 if p.is_file() and p.suffix in {'.json','.log','.py','.ps1','.sh','.stdout','.stderr','.txt'} and p.name!='fmt-original.log':copy(p,p.relative_to(source))
for p in sorted((root/'target/sdk-table-batches-validation/macos-ui').iterdir()):
 if p.is_file() and p.suffix in {'.json','.stdout','.stderr'}:copy(p,Path('macos-ui')/p.name)
copy(root/'target/sdk-table-batches-validation/linux-launch.sh',Path('runtime/linux-launch.sh'))
(dest/'log-bindings.json').write_text(json.dumps(rows,indent=2)+'\n')
for r in json.loads((source/'source-bindings.json').read_text()):assert sha((root/r['path']).read_bytes())==r['sha256'],r['path']
paths=[root/'target/test-tools/dartforge-before-sdk-table-batches',root/'target/test-tools/dartforge-sdk-table-batches']+[p for p in (root/'target/sdk-table-batches-validation').rglob('*') if p.is_file()]
artifacts=[]
for p in sorted(set(paths)):
 h=hashlib.sha256()
 with p.open('rb') as f:
  for chunk in iter(lambda:f.read(1024*1024),b''):h.update(chunk)
 artifacts.append(dict(path=str(p.relative_to(root)),bytes=p.stat().st_size,sha256=h.hexdigest()))
(dest/'artifact-bindings.json').write_text(json.dumps(artifacts,indent=2)+'\n');print('Stored',len(rows),'logs/scripts and',len(artifacts),'artifact bindings')

from pathlib import Path
import hashlib,json,os,shutil
root=Path.cwd(); dest=root/'bench/results/2026-10-20-table-batches';dest.mkdir(exist_ok=True)
raw=root/'target/batch-tables-validation'; logs=[]
def digest(data):return hashlib.sha256(data).hexdigest()
def copy(source,target):
 data=source.read_bytes(); original=data
 if source.suffix in {'.log','.stderr','.stdout','.txt','.json','.py','.ps1','.sh'}:
  normalized='\n'.join(line.rstrip() for line in data.decode('utf-16' if data.startswith((b'\xff\xfe',b'\xfe\xff')) else 'utf-8-sig').splitlines()).rstrip('\n')
  data=(normalized+'\n').encode() if normalized else b''
 target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(data)
 logs.append(dict(source=str(source),stored=str(target.relative_to(dest)),original_bytes=len(original),original_sha256=digest(original),stored_bytes=len(data),stored_sha256=digest(data)))
source=Path('/tmp/forge-batch-tables-runtime')
for p in sorted(source.iterdir()):
 if p.is_file():copy(p,dest/'runtime'/p.name)
copy(Path('/tmp/forge-batch-tables-final.py'),dest/'validate-checks.py')
copy(Path('/tmp/collect-table-batches.py'),dest/'collect-evidence.py')
for p in sorted(raw.glob('linux-*.log')):copy(p,dest/'guest-builds'/p.name)
for p in Path('/tmp/forge-batch-tables-final').iterdir():
 if p.name!='fmt-original.log':copy(p,dest/p.name)
for p in sorted((raw/'macos-ui').iterdir()):
 if p.suffix in {'.json','.stderr','.stdout'}:copy(p,dest/'macos-ui'/p.name)
for p in sorted(raw.glob('windows-*.log')):copy(p,dest/'guest-builds'/p.name)
for p in sorted(raw.glob('windows-*.ps1')):copy(p,dest/'runtime'/p.name)
(dest/'log-bindings.json').write_text(json.dumps(logs,indent=2)+'\n')
for binding in json.loads((source/'source-bindings-final.json').read_text()):
 assert digest((root/binding['path']).read_bytes())==binding['sha256'],binding
artifacts=[]
files=[raw/'before.ll',raw/'after.ll',raw/'source-snapshot.tar.gz',raw/'source-snapshot-final.tar.gz']
files += list((root/'target/test-tools').glob('*batch*'))+[root/'target/test-tools/dartforge-proof-views',root/'target/test-tools/emitter-build-script-before',root/'target/test-tools/emitter-build-script-after']
files += [p for p in raw.iterdir() if p.is_file() and p.suffix not in {'.json','.ll','.stdout','.stderr','.log','.py','.ps1','.sh','.gz'}]
files += [raw/'macos-ui/explorer',raw/'macos-ui/context-smoke']
for target in ['linux-arm64','windows-x64']:
 bundle=root/'target/cross-bundles/batch-tables'/target
 manifest=json.loads((bundle/'target.json').read_text());assert 'a37848c6491b54570a07e4ca916ece5310d20e4c435fc32258dd2045db587ee5' in json.dumps(manifest)
 files += [p for p in bundle.rglob('*') if p.is_file()]
for p in sorted(set(files)):
 if p.is_file():
  h=hashlib.sha256()
  with p.open('rb') as f:
   for chunk in iter(lambda:f.read(1024*1024),b''):h.update(chunk)
  artifacts.append(dict(path=str(p.relative_to(root)),bytes=p.stat().st_size,sha256=h.hexdigest()))
(dest/'artifact-bindings.json').write_text(json.dumps(artifacts,indent=2)+'\n')
print('Stored',len(logs),'logs/scripts and',len(artifacts),'artifact bindings')

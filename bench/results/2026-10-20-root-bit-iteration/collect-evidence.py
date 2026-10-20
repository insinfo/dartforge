from pathlib import Path
import json,hashlib,gzip
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');source=Path('/tmp/forge-root-bit-iteration');dest=root/'bench/results/2026-10-20-root-bit-iteration';dest.mkdir(parents=True,exist_ok=True)
manifest=[]
for path in sorted(source.rglob('*')):
 if not path.is_file() or path.suffix not in {'.json','.log','.py','.rs'}:continue
 relative=path.relative_to(source);data=path.read_bytes();stored=('\n'.join(line.rstrip() for line in data.decode().splitlines()).rstrip()+'\n').encode();target=dest/relative;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(stored)
 manifest.append(dict(path=str(relative),original_sha256=hashlib.sha256(data).hexdigest(),stored_sha256=hashlib.sha256(stored).hexdigest()))
profile=dest/'profile';profile.mkdir(exist_ok=True)
for name in ['state.json','compile.log','sample-2.txt','sample-2-demangled.txt']:
 data=(Path('/tmp/forge-explorer-profile/late')/name).read_bytes();compressed=gzip.compress(data,mtime=0);target=profile/(name+'.gz');target.write_bytes(compressed);manifest.append(dict(path=str(target.relative_to(dest)),original_sha256=hashlib.sha256(data).hexdigest(),stored_sha256=hashlib.sha256(compressed).hexdigest(),encoding='gzip'))
(dest/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(len(manifest),'retained evidence files')

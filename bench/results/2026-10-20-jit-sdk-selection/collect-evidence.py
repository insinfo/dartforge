from pathlib import Path
import json,hashlib,subprocess,shutil,gzip
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');source=root/'target/current-desktop-validation';dest=root/'bench/results/2026-10-20-jit-sdk-selection';dest.mkdir(parents=True,exist_ok=True)
manifest=[]
for path in sorted(source.rglob('*')):
 if not path.is_file() or path.suffix not in {'.json','.log','.py','.ps1','.sh','.diff','.stdout','.stderr'}:continue
 if path.suffix=='.json' and path.name.endswith('.contracts.json'):continue
 if any(part in {'.df_tmp','invalid-sdk'} for part in path.relative_to(source).parts):continue
 relative=path.relative_to(source);data=path.read_bytes()
 if path.suffix=='.diff':
  stored=gzip.compress(data,mtime=0);target=dest/(str(relative)+'.gz');target.write_bytes(stored);(dest/relative).unlink(missing_ok=True);manifest.append(dict(path=str(relative)+'.gz',original_sha256=hashlib.sha256(data).hexdigest(),stored_sha256=hashlib.sha256(stored).hexdigest(),input_encoding='utf-8',storage_encoding='gzip'));continue
 encoding='utf-16' if data.startswith((b'\xff\xfe',b'\xfe\xff')) else 'utf-8-sig'
 text=data.decode(encoding);stored=('\n'.join(line.rstrip() for line in text.splitlines()).rstrip()+'\n').encode() if text else b''
 target=dest/relative;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(stored)
 manifest.append(dict(path=str(relative),original_sha256=hashlib.sha256(data).hexdigest(),stored_sha256=hashlib.sha256(stored).hexdigest(),input_encoding=encoding))
for name,path in [('sdk-check.py',Path('/tmp/forge-explicit-jit-sdk-check.py')),('final-checks.py',Path('/tmp/forge-explicit-jit-sdk-final-checks.py')),('macos-jit-fixed.py',Path('/tmp/forge-current-macos-fixed.py')),('matrix-audit.py',Path('/tmp/forge-current-matrix-audit.py')),('collect-evidence.py',Path(__file__))]:
 data=path.read_bytes();(dest/name).write_bytes(data);manifest.append(dict(path=name,original_sha256=hashlib.sha256(data).hexdigest(),stored_sha256=hashlib.sha256(data).hexdigest(),input_encoding='utf-8'))
(dest/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n');print(len(manifest),'evidence files retained')

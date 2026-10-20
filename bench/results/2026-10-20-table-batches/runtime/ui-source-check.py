from pathlib import Path
import hashlib,json,sys
root=Path(sys.argv[1]);files=[]
for directory in ['lib','examples/file_explorer']:
 for file in (root/directory).rglob('*.dart'):
  if not any(part in {'.dart_tool','build','.git'} for part in file.relative_to(root).parts):files.append(file)
hash=hashlib.sha256()
for file in sorted(files, key=lambda file: file.relative_to(root).as_posix()):
 hash.update(file.relative_to(root).as_posix().encode());hash.update(b'\0');hash.update(file.read_bytes().replace(b'\r\n',b'\n'));hash.update(b'\0')
print(json.dumps(dict(files=len(files),normalized_dart_sha256=hash.hexdigest())))

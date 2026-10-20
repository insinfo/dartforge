from pathlib import Path
import hashlib,json,sys
ui=Path(sys.argv[1]);sdk=Path(sys.argv[2]);out=Path(sys.argv[3])
def digest_files(root,files):
 digest=hashlib.sha256()
 for path in sorted(files,key=lambda p:p.relative_to(root).as_posix()):
  digest.update(path.relative_to(root).as_posix().encode());digest.update(b'\0');digest.update(path.read_bytes().replace(b'\r\n',b'\n'));digest.update(b'\0')
 return dict(files=len(files),normalized_sha256=digest.hexdigest())
files=[p for directory in ['lib','examples/file_explorer'] for p in (ui/directory).rglob('*.dart') if not any(part in {'.dart_tool','build','.git'} for part in p.relative_to(ui).parts)]
locks={str(p.relative_to(ui)):hashlib.sha256(p.read_bytes().replace(b'\r\n',b'\n')).hexdigest() for directory in [ui,ui/'examples/file_explorer'] for p in [directory/'pubspec.yaml',directory/'pubspec.lock']}
report=dict(dart=digest_files(ui,files),locks=locks,sdk=digest_files(sdk, list((sdk/'lib').rglob('*.dart'))+list((sdk/'lib').rglob('libraries.json'))),sdk_version=(sdk/'version').read_text().strip())
out.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))

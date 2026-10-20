from pathlib import Path
import subprocess,json,hashlib,time
root=Path.cwd();fixture=root/'target/relr-validation';out=root/'target/relr-explorer'
args=json.loads((fixture/'packed.command.json').read_text());assert args[1:3]==['-z','pack-relative-relocs'];raw=['/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/ld.lld','--pack-dyn-relocs=relr']+args[3:];raw[raw.index('-o')+1]=str(fixture/'unversioned')
r=subprocess.run(raw,capture_output=True,text=True);assert r.returncode==0,r.stderr
assert b'GLIBC_ABI_DT_RELR' not in (fixture/'unversioned').read_bytes()
(fixture/'unversioned.command.json').write_text(json.dumps(raw,indent=2)+'\n')
script=out/'run-user.sh';script.write_text('set -eu\ncd /home/isaque/projects/dart_ui/examples/file_explorer\nexec "$1" /home/isaque/projects/dart_ui/examples/file_explorer --lang=pt --backend=wayland --presentation=wayland-opengl --frames=2\n')
vm='Ubuntu24.04_arm64';results=[]
def call(argv):return subprocess.run(['prlctl','exec',vm]+argv,capture_output=True,text=True)
libc=call(['/usr/bin/getconf','GNU_LIBC_VERSION']);print(libc.stdout.strip(),flush=True)
for category,base in [('fixture',fixture),('explorer',out)]:
 for mode in ['standard','packed']:
  source=base/mode;guest='/tmp/dartforge-relr-'+category+'-'+mode
  shared='/media/psf/ssd_dock/MyRustProjects/dartforge/'+str(source.relative_to(root))
  r=call(['/bin/cp',shared,guest]);assert r.returncode==0,r.stderr
  r=call(['/bin/chmod','755',guest]);assert r.returncode==0,r.stderr
  expected=hashlib.sha256(source.read_bytes()).hexdigest();remote=call(['/usr/bin/sha256sum',guest]);assert remote.stdout.split()[0]==expected
  argv=[guest] if category=='fixture' else ['/usr/sbin/runuser','-u','isaque','--','/usr/bin/env','XDG_RUNTIME_DIR=/run/user/1000','WAYLAND_DISPLAY=wayland-0','XDG_SESSION_TYPE=wayland','DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus','DART_UI_FONTS=/home/isaque/projects/dart_ui/assets/fonts','/bin/bash','/media/psf/ssd_dock/MyRustProjects/dartforge/target/relr-explorer/run-user.sh',guest]
  start=time.monotonic();r=call(argv);record={'category':category,'mode':mode,'command':argv,'exit':r.returncode,'stdout':r.stdout,'stderr':r.stderr,'seconds':time.monotonic()-start,'sha256':expected,'glibc':libc.stdout.strip()};results.append(record);(out/'vm-results.json').write_text(json.dumps(results,indent=2)+'\n');print(json.dumps(record),flush=True)
  assert r.returncode==0,r.stderr
  if category=='fixture':assert r.stdout.strip()=='relr-ok'
source=fixture/'unversioned';guest='/tmp/dartforge-relr-unversioned'
r=call(['/bin/cp','/media/psf/ssd_dock/MyRustProjects/dartforge/target/relr-validation/unversioned',guest]);assert r.returncode==0
call(['/bin/chmod','755',guest]);r=call([guest]);assert r.returncode!=0 and 'GLIBC_ABI_DT_RELR' in (r.stdout+r.stderr)
control={'category':'negative-unversioned','exit':r.returncode,'stdout':r.stdout,'stderr':r.stderr,'sha256':hashlib.sha256(source.read_bytes()).hexdigest()};(out/'unversioned-control.json').write_text(json.dumps(control,indent=2)+'\n');print(json.dumps(control),flush=True)
versions=call(['/usr/bin/readelf','--version-info','/tmp/dartforge-relr-explorer-packed']);(out/'version-info.log').write_text(versions.stdout+versions.stderr);assert versions.returncode==0 and 'GLIBC_ABI_DT_RELR' in versions.stdout

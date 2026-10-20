from pathlib import Path
import os,subprocess,json,time,hashlib,shutil
root=Path('/media/psf/ssd_dock/MyRustProjects/dartforge');shared=root/'target/sdk-table-batches-validation';out=Path.home()/'.cache/dartforge/sdk-table-batches';out.mkdir(parents=True,exist_ok=True);ui=Path('/home/isaque/projects/dart_ui');example=ui/'examples/file_explorer';artifacts={};rows=[]
for kind in ['fixture','explorer','context']:
 source=shared/f'linux-arm64-{kind}';dest=out/kind;shutil.copy2(source,dest);dest.chmod(0o755);sha=hashlib.sha256(dest.read_bytes()).hexdigest();assert sha==hashlib.sha256(source.read_bytes()).hexdigest();artifacts[kind]=dict(bytes=dest.stat().st_size,sha256=sha)
env=os.environ.copy();env.update(XDG_RUNTIME_DIR=f'/run/user/{os.getuid()}',WAYLAND_DISPLAY='wayland-0',DBUS_SESSION_BUS_ADDRESS=f'unix:path=/run/user/{os.getuid()}/bus',DART_UI_FONTS=str(ui/'assets/fonts'));env.pop('DISPLAY',None);env.pop('DARTFORGE_GC_STRESS',None)
commands=[('fixture-normal',[str(out/'fixture')],False,'parent=45\nchild=45\n'),('fixture-stress',[str(out/'fixture')],True,'parent=45\nchild=45\n'),('explorer-aot',[str(out/'explorer'),str(example),'--lang=pt','--frames=2','--backend=wayland','--presentation=wayland-opengl'],False,None),('context-aot',[str(out/'context'),'--backend=wayland','--presentation=wayland-opengl'],False,'CONTEXT_RETARGET=PASS')]
for name,command,stress,marker in commands:
 runtime=env.copy()
 if stress:runtime['DARTFORGE_GC_STRESS']='1'
 start=time.monotonic();p=subprocess.run(command,cwd=example,env=runtime,capture_output=True,text=True);r=dict(name=name,command=command,exit=p.returncode,seconds=time.monotonic()-start,stdout=p.stdout,stderr=p.stderr,gc_stress=stress);rows.append(r);report=dict(artifacts=artifacts,runs=rows);(out/'results.json').write_text(json.dumps(report,indent=2)+'\n');print(name,p.returncode,flush=True)
 assert p.returncode==0 and not p.stderr,r
 if name.startswith('fixture'):assert p.stdout==marker,r
 elif marker:assert marker in p.stdout,r
print(json.dumps(report),flush=True)

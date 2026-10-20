from pathlib import Path
import os,subprocess,json,time,hashlib,shutil
root=Path('/media/psf/ssd_dock/MyRustProjects/dartforge');shared=root/'target/batch-tables-validation';out=Path.home()/'.cache/dartforge/table-batches-ui';out.mkdir(parents=True,exist_ok=True);ui=Path('/home/isaque/projects/dart_ui');example=ui/'examples/file_explorer';compiler=Path('/home/isaque/projects/dartforge/target/native-validation/release/dartforge-batch-tables');sdk='/opt/dart/3.6.2/dart-sdk/lib';records=[]
env=os.environ.copy();env.update(XDG_RUNTIME_DIR=f'/run/user/{os.getuid()}',WAYLAND_DISPLAY='wayland-0',DISPLAY=':0',DBUS_SESSION_BUS_ADDRESS=f'unix:path=/run/user/{os.getuid()}/bus',DART_UI_FONTS=str(ui/'assets/fonts'),DARTFORGE_LLVM_DIR='/opt/llvm-22.1.8');env.pop('DARTFORGE_GC_STRESS',None);env['PATH']='/opt/llvm-22.1.8/bin:'+env['PATH']
report=dict(compiler=str(compiler),compiler_sha256=hashlib.sha256(compiler.read_bytes()).hexdigest(),runs=records)
for label in ['before','after']:
 source=shared/f'linux-arm64-{label}-explorer';dest=out/f'explorer-{label}';shutil.copy2(source,dest);dest.chmod(0o755);assert hashlib.sha256(source.read_bytes()).digest()==hashlib.sha256(dest.read_bytes()).digest()
source=shared/'linux-arm64-after-context';dest=out/'context-after';shutil.copy2(source,dest);dest.chmod(0o755)
commands=[('explorer-before-aot',[str(out/'explorer-before'),str(example),'--lang=pt','--frames=2','--backend=wayland','--presentation=opengl'],None),('explorer-after-aot',[str(out/'explorer-after'),str(example),'--lang=pt','--frames=2','--backend=wayland','--presentation=opengl'],None),('context-after-aot',[str(out/'context-after'),'--backend=wayland','--presentation=opengl'],'CONTEXT_RETARGET=PASS')]
for kind,entry in [('explorer',example/'bin/file_explorer.dart'),('context',example/'tool/context_menu_retarget_smoke.dart')]:
 args=[str(example),'--lang=pt','--frames=2'] if kind=='explorer' else []
 commands.append((kind+'-after-jit',[str(compiler),'run',str(entry),'--sdk',sdk,'--packages',str(example/'.dart_tool/package_config.json'),'--',*args,'--backend=wayland','--presentation=opengl'],None if kind=='explorer' else 'CONTEXT_RETARGET=PASS'))
for name,command,marker in commands:
 start=time.monotonic();p=subprocess.run(command,cwd=example,env=env,capture_output=True,text=True);r=dict(name=name,command=command,exit=p.returncode,seconds=time.monotonic()-start,stdout=p.stdout,stderr=p.stderr);records.append(r);(out/'results.json').write_text(json.dumps(report,indent=2)+'\n');print(name,p.returncode,flush=True)
 assert p.returncode==0 and (marker is None or marker in p.stdout),r
 assert 'Bad state:' not in p.stderr and 'Unhandled exception' not in p.stderr,r
print(json.dumps(report),flush=True)

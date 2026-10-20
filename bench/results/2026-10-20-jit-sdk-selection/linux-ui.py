from pathlib import Path
import os,subprocess,json,time,hashlib,shutil,sys
mode=sys.argv[1] if len(sys.argv)>1 else 'all'
assert mode in {'aot','jit','all'}
shared=Path('/media/psf/ssd_dock/MyRustProjects/dartforge/target/current-desktop-validation');out=Path.home()/'.cache/dartforge/current-desktop-ui';out.mkdir(parents=True,exist_ok=True)
ui=Path('/home/isaque/projects/dart_ui');example=ui/'examples/file_explorer';compiler=Path('/home/isaque/projects/dartforge/target/native-validation/release/dartforge-current-desktop-fixed');sdk='/opt/dart/3.6.2/dart-sdk/lib'
env=os.environ.copy();env.update(XDG_RUNTIME_DIR=f'/run/user/{os.getuid()}',WAYLAND_DISPLAY='wayland-0',DBUS_SESSION_BUS_ADDRESS=f'unix:path=/run/user/{os.getuid()}/bus',DART_UI_FONTS=str(ui/'assets/fonts'),DARTFORGE_LLVM_DIR='/opt/llvm-22.1.8',DARTFORGE_SDK_LIB=str(shared/'invalid-sdk'));env['PATH']='/opt/llvm-22.1.8/bin:'+env['PATH']
for key in ['DISPLAY','DART_SDK','DARTFORGE_SDK_DLL','DARTFORGE_CLOSED_CLASS_REACHABILITY','DARTFORGE_GC_STRESS','DARTFORGE_NO_PRUNING','DARTFORGE_NO_TABLE_PRUNING']:env.pop(key,None)
rows=[];report=dict(compiler=str(compiler),compiler_sha256=hashlib.sha256(compiler.read_bytes()).hexdigest(),runs=rows,transfers=[],ambient_sdk=env['DARTFORGE_SDK_LIB'])
for kind in ([] if mode=='jit' else ['explorer','context']):
 source=shared/('linux-arm64-'+kind);dest=out/kind;shutil.copy2(source,dest);dest.chmod(0o755)
 digest=hashlib.sha256(source.read_bytes()).hexdigest();assert digest==hashlib.sha256(dest.read_bytes()).hexdigest()
 report['transfers'].append(dict(source=str(source),destination=str(dest),sha256=digest))
commands=[]
for kind,entry in [('explorer','bin/file_explorer.dart'),('context','tool/context_menu_retarget_smoke.dart')]:
 args=[str(example),'--lang=pt','--frames=2'] if kind=='explorer' else []
 args+=['--backend=wayland','--presentation=wayland-opengl'];marker=None if kind=='explorer' else 'CONTEXT_RETARGET=PASS'
 if mode!='jit':commands.append((kind+'-cross-aot',[str(out/kind),*args],marker))
 if mode!='aot':commands.append((kind+'-native-jit',[str(compiler),'run','--sdk',sdk,'--packages',str(example/'.dart_tool/package_config.json'),str(example/entry),'--',*args],marker))
for name,command,marker in commands:
 start=time.monotonic();p=subprocess.Popen(command,cwd=example,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
 report['state']=dict(status='running',name=name,pid=p.pid);(shared/f'linux-ui-{mode}-report.json').write_text(json.dumps(report,indent=2)+'\n')
 stdout,stderr=p.communicate();r=dict(name=name,command=command,exit=p.returncode,seconds=time.monotonic()-start,stdout=stdout.decode(),stderr=stderr.decode());rows.append(r)
 (shared/f'linux-ui-{mode}-report.json').write_text(json.dumps(report,indent=2)+'\n');print(name,p.returncode,flush=True)
 assert p.returncode==0 and (marker is None or marker in r['stdout']),r
 assert not r['stderr'],r
report['state']=dict(status='finished');(shared/f'linux-ui-{mode}-report.json').write_text(json.dumps(report,indent=2)+'\n')

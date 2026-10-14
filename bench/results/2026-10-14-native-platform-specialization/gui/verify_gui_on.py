import pathlib,subprocess,os,json,time,hashlib,signal
root=pathlib.Path(__file__).resolve().parents[2];base=root/'target/current-file-explorer-validation';out=root/'target/platform-specialization/gui-on';out.mkdir(exist_ok=False);ui=pathlib.Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui');fixture=root/'target/file-explorer-fixture';binary=root/'target/platform-specialization/sizes/on/program';sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
entry=ui/'examples/file_explorer/bin/file_explorer.dart';before=sha(entry);manifest={'artifact':str(binary),'artifact_sha256':sha(binary),'cwd':str(ui),'arguments':[str(fixture),'--lang=en'],'entry_source_sha256':before,'actions':[],'visual_review_complete':False,'performance_claim':False}
with (out/'stdout').open('wb') as stdout,(out/'stderr').open('wb') as stderr:
 p=subprocess.Popen([str(binary),*manifest['arguments']],cwd=ui,env=dict(os.environ,TMPDIR=str(out)),stdout=stdout,stderr=stderr,start_new_session=True);manifest['pid']=p.pid
 try:
  time.sleep(3)
  for action in ['capture','navigate','back-button','close-button']:
   command=['python3',str(base/'window_control.py'),'--pid',str(p.pid),'--output',str(out),'--action',action]
   if action=='navigate':command+=['--folder',str(fixture/'dartforge-child')]
   started=time.time();r=subprocess.run(command,capture_output=True,timeout=25);row={'action':action,'command':command,'started_unix':started,'ended_unix':time.time(),'exit_code':r.returncode,'stdout':r.stdout.decode(),'stderr':r.stderr.decode(),'application_poll':p.poll()};manifest['actions'].append(row);(out/'validation.json').write_text(json.dumps(manifest,indent=2)+'\n');print(action,r.returncode,flush=True)
   if r.returncode:raise RuntimeError('Actual UI action failed: '+action)
  manifest['exit_code']=p.wait(timeout=20);assert manifest['exit_code']==0
 finally:
  if p.poll() is None:
   manifest['required_forced_cleanup']=True;os.killpg(p.pid,signal.SIGTERM);manifest['cleanup_exit_code']=p.wait(timeout=10)
  (out/'validation.json').write_text(json.dumps(manifest,indent=2)+'\n')
manifest.update({'entry_source_unchanged':sha(entry)==before,'artifact_unchanged':sha(binary)==manifest['artifact_sha256'],'stdout_bytes':(out/'stdout').stat().st_size,'stderr_bytes':(out/'stderr').stat().st_size,'screenshots_sha256':{f.name:sha(f) for f in out.glob('*.png')}});(out/'validation.json').write_text(json.dumps(manifest,indent=2)+'\n');assert manifest['entry_source_unchanged'] and manifest['artifact_unchanged'];print('NATURAL_EXIT='+str(manifest['exit_code']),flush=True)

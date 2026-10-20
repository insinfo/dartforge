from pathlib import Path
import subprocess,json
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-context-footprints/runtime')
command=['prlctl','exec','Windows11','--current-user','powershell','-ExecutionPolicy','Bypass','-File',r'\\Mac\ssd_dock\MyRustProjects\dartforge\target\context-calls-validation\windows-run.ps1']
r=subprocess.run(command,capture_output=True,text=True)
(out/'windows-launch.json').write_text(json.dumps(dict(command=command,exit=r.returncode,stdout=r.stdout,stderr=r.stderr),indent=2)+'\n')
print(r.returncode,r.stdout,r.stderr,flush=True);assert r.returncode==0

from pathlib import Path
import subprocess,json,sys
out=Path('/tmp/forge-read-only-allocation/runtime');kind=sys.argv[1];vm='Ubuntu24.04_arm64' if kind=='linux' else 'Windows11';rows=[]
def call(command):
 r=subprocess.run(command,capture_output=True,text=True)
 rows.append(dict(command=command,exit=r.returncode,stdout=r.stdout,stderr=r.stderr));(out/(kind+'-launch.json')).write_text(json.dumps(rows,indent=2)+'\n');print(command[1],r.returncode,r.stdout,r.stderr,flush=True);assert r.returncode==0
call(['prlctl','start',vm])
try:
 if kind=='linux':call(['prlctl','exec',vm,'python3','/media/psf/ssd_dock/MyRustProjects/dartforge/target/read-only-allocation-validation/linux-run.py'])
 else:call(['prlctl','exec',vm,'--current-user','powershell','-ExecutionPolicy','Bypass','-File',r'\\Mac\ssd_dock\MyRustProjects\dartforge\target\read-only-allocation-validation\windows-run.ps1'])
finally:
 call(['prlctl','suspend',vm])

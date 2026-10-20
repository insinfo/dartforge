from pathlib import Path
import subprocess,json,time,shlex,sys
out=Path('/tmp/forge-batch-tables-runtime');osname=sys.argv[1]
if osname.startswith('linux'):command=['prlctl','exec','Ubuntu24.04_arm64','/bin/bash','-lc',shlex.quote('bash /media/psf/ssd_dock/MyRustProjects/dartforge/target/batch-tables-validation/build-linux-release-cli.sh')]
else:command=['prlctl','exec','Windows11','--current-user','powershell.exe','-NoLogo','-NoProfile','-NonInteractive','-OutputFormat','Text','-ExecutionPolicy','Bypass','-File',r'\\Mac\ssd_dock\MyRustProjects\dartforge\target\batch-tables-validation\build-windows-release-cli.ps1']
start=time.monotonic();p=subprocess.run(command,capture_output=True,text=True);record=dict(command=command,exit=p.returncode,seconds=time.monotonic()-start,stdout=p.stdout,stderr=p.stderr);(out/('build-release-cli-'+osname+'-result.json')).write_text(json.dumps(record,indent=2)+'\n');print(osname,p.returncode,p.stdout[-1200:],p.stderr[-1000:],flush=True);assert p.returncode==0

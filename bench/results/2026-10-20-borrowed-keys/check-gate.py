from pathlib import Path
import os,subprocess,json
s=Path('/tmp/forge-borrowed-keys/check.py').read_text();exec(s[:s.index('commands=')])
env['DARTFORGE_NATIVE_CACHE']=str(root/'target/rpo-native-cache')
command=['cargo','test','--locked','-p','dartforge-emit-native','--test','production_profile','platform_constant_prunes_the_other_backends','--','--include-ignored','--nocapture']
with (out/'foreign-backend-gate.log').open('w') as log:r=subprocess.run(command,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
(out/'foreign-backend-gate.json').write_text(json.dumps(dict(command=command,exit=r.returncode),indent=2)+'\n');print('Foreign-backend gate',r.returncode,flush=True)

runuser -u isaque -- /usr/bin/python3 - <<'PYCODE'
import os,pathlib,hashlib,shutil,subprocess,json
source=pathlib.Path('/media/psf/ssd_dock/MyRustProjects/dartforge/target/sdk-image-owner-validation/cross-linux-arm64');dest=pathlib.Path.home()/'.cache/dartforge/sdk-image-owner'/'6302fbd2fce9140a7f29c02784d8ec1753f1d18001686a1af0c0e1de250800d0'
assert hashlib.sha256(source.read_bytes()).hexdigest()=='6302fbd2fce9140a7f29c02784d8ec1753f1d18001686a1af0c0e1de250800d0'
dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(source,dest);dest.chmod(0o755)
rows=[]
for stress in [False,True]:
 env=os.environ.copy();env.pop('DARTFORGE_GC_STRESS',None)
 if stress:env['DARTFORGE_GC_STRESS']='1'
 result=subprocess.run([str(dest)],env=env,capture_output=True,text=True)
 rows.append(dict(stress=stress,exit=result.returncode,stdout=result.stdout,stderr=result.stderr,sha256=hashlib.sha256(dest.read_bytes()).hexdigest()))
print(json.dumps(rows,ensure_ascii=False))
PYCODE

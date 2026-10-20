runuser -u isaque -- /usr/bin/python3 - <<'PYCODE'
import os,pathlib,hashlib,shutil,subprocess,json
source=pathlib.Path('/media/psf/ssd_dock/MyRustProjects/dartforge/target/batch-tables-validation/cross-linux-arm64');dest=pathlib.Path.home()/'.cache/dartforge/sdk-image-owner'/'007f40f76c73809b91edbd838c19eccad165bed38597e3b85006af46a9aeb543'
assert hashlib.sha256(source.read_bytes()).hexdigest()=='007f40f76c73809b91edbd838c19eccad165bed38597e3b85006af46a9aeb543'
dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(source,dest);dest.chmod(0o755)
rows=[]
for stress in [False,True]:
 env=os.environ.copy();env.pop('DARTFORGE_GC_STRESS',None)
 if stress:env['DARTFORGE_GC_STRESS']='1'
 result=subprocess.run([str(dest)],env=env,capture_output=True,text=True)
 rows.append(dict(stress=stress,exit=result.returncode,stdout=result.stdout,stderr=result.stderr,sha256=hashlib.sha256(dest.read_bytes()).hexdigest()))
print(json.dumps(rows,ensure_ascii=False))
PYCODE

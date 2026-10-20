from pathlib import Path
import subprocess,json,hashlib,os,base64
shared=Path('/media/psf/ssd_dock/MyRustProjects/dartforge/target/stream-verify-validation')
source=shared/'after-linux-arm64-fixture';dest=Path('/tmp/dartforge-stream-verify-fixture');dest.write_bytes(source.read_bytes());dest.chmod(0o755)
try:
 sha=hashlib.sha256(dest.read_bytes()).hexdigest();assert sha==hashlib.sha256(source.read_bytes()).hexdigest()
 expected=base64.b64decode('cGFyZW50PTQ1CmNoaWxkPTQ1Cg==').decode();rows=[]
 for stress in [False,True]:
  env=os.environ.copy();env.pop('DARTFORGE_GC_STRESS',None)
  if stress:env['DARTFORGE_GC_STRESS']='1'
  p=subprocess.run([str(dest)],env=env,capture_output=True,text=True)
  rows.append(dict(exit=p.returncode,stdout=p.stdout,stderr=p.stderr,stress=stress,sha256=sha))
  (shared/'linux-result.json').write_text(json.dumps(rows,indent=2)+'\n')
  assert p.returncode==0 and p.stdout==expected and not p.stderr,rows[-1]
 print(json.dumps(rows))
finally:
 dest.unlink(missing_ok=True)

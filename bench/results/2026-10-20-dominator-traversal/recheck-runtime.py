from pathlib import Path
import os,subprocess,json,hashlib
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-rpo-validation');records=[]
expected=json.loads((out/'comparison.json').read_text())['reference_stdout']
for label in ['before','after']:
 executable=root/'target/rpo-validation'/('branches-'+label)
 for stress in [False,True]:
  environment=os.environ.copy();environment.pop('DARTFORGE_GC_STRESS',None)
  if stress:environment['DARTFORGE_GC_STRESS']='1'
  result=subprocess.run([str(executable)],env=environment,capture_output=True,text=True)
  record=dict(label=label,command=[str(executable)],executable_sha256=hashlib.sha256(executable.read_bytes()).hexdigest(),environment={'DARTFORGE_GC_STRESS':environment.get('DARTFORGE_GC_STRESS')},exit=result.returncode,stdout=result.stdout,stderr=result.stderr,matches_vm=result.stdout==expected)
  records.append(record);(out/'runtime-recheck.json').write_text(json.dumps(records,indent=2)+'\n')
  assert result.returncode==0 and result.stdout==expected and not result.stderr,record
print('Four explicit normal/stressed executions match Dart VM output.')

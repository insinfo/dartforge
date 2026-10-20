from pathlib import Path
import os,subprocess,json,time,hashlib
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-rpo-validation');artifacts=root/'target/rpo-validation';prefix='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64'
env=os.environ.copy();env.update(SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk',MACOSX_DEPLOYMENT_TARGET='15.0',DARTFORGE_NATIVE_CACHE=str(root/'target/rpo-native-cache'));env['PATH']=prefix+'/bin:/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin:'+env['PATH']
before=root/'target/test-tools/dartforge-rpo-before';after=root/'target/test-tools/dartforge-rpo-after';records=[]
def digest(path):
 h=hashlib.sha256()
 with path.open('rb') as stream:
  for chunk in iter(lambda:stream.read(1<<20),b''):h.update(chunk)
 return h.hexdigest()
def run(label,command,environment=None):
 start=time.monotonic();result=subprocess.run(command,cwd=root,env=environment or env,capture_output=True,text=True)
 (out/(label+'.stdout')).write_text(result.stdout);(out/(label+'.stderr')).write_text(result.stderr)
 records.append(dict(label=label,command=list(map(str,command)),exit=result.returncode,seconds=time.monotonic()-start,stdout=result.stdout,stderr=result.stderr))
 (out/'execution.json').write_text(json.dumps(records,indent=2)+'\n');print(label,result.returncode,round(records[-1]['seconds'],3),flush=True)
 assert result.returncode==0,(label,result.stderr)
 return result
entry='/Volumes/ssd_dock/MyProjectsDart/dart_ui/examples/file_explorer/bin/file_explorer.dart';packages='/Volumes/ssd_dock/MyProjectsDart/dart_ui/examples/file_explorer/.dart_tool/package_config.json'
metadata=dict(before_compiler_sha256=digest(before),after_compiler_sha256=digest(after),explorer_entry=entry,entry_sha256=digest(Path(entry)),packages_sha256=digest(Path(packages)),dart_ui_revision=subprocess.check_output(['git','-C','/Volumes/ssd_dock/MyProjectsDart/dart_ui','rev-parse','HEAD'],text=True).strip(),dart_ui_status=subprocess.check_output(['git','-C','/Volumes/ssd_dock/MyProjectsDart/dart_ui','status','--short'],text=True))
assert metadata['before_compiler_sha256']!=metadata['after_compiler_sha256']
command=[after,'compile-native',entry,'--optimize','--emit-ir','--digest','--timings','--packages',packages,'--sdk','/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib','-o',artifacts/'explorer-after.ll']
expected=digest(artifacts/'explorer-before.ll')
for warm in [False,True]:
 run('explorer-after-warm' if warm else 'explorer-after-cold',command)
 actual=digest(artifacts/'explorer-after.ll');metadata['explorer_ir_sha256']=actual;metadata['explorer_ir_equal']=actual==expected
 (out/'comparison.json').write_text(json.dumps(metadata,indent=2)+'\n')
 assert actual==expected,dict(expected=expected,actual=actual)
branch=root/'target/dense-dominator-validation/branches.dart';reference=run('dart-vm',['dart',str(branch)]).stdout
for label,compiler in [('before',before),('after',after)]:
 executable=artifacts/('branches-'+label)
 run(label+'-compile',[compiler,'compile-native',branch,'--optimize','--sdk','/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib','-o',executable])
 for stress in [False,True]:
  runtime=env.copy()
  if stress:runtime['DARTFORGE_GC_STRESS']='1'
  result=run(label+('-stress' if stress else '-normal'),[executable],runtime)
  assert result.stdout==reference,(label,stress,result.stdout,reference)
metadata.update(branch_source_sha256=digest(branch),reference_stdout=reference,all_executions_match_vm=True,explorer_before_path=str(artifacts/'explorer-before.ll'),explorer_after_path=str(artifacts/'explorer-after.ll'),explorer_ir_bytes=(artifacts/'explorer-after.ll').stat().st_size)
(out/'comparison.json').write_text(json.dumps(metadata,indent=2)+'\n')

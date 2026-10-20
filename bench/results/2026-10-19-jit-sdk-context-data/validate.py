from pathlib import Path
import os,subprocess,json,hashlib
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-jit-sdk-context');fixture=Path('/tmp/forge-string-length-reuse');runner=root/'target/test-tools/dartforge-run-ir-sdk-data-fixed';before=root/'target/test-tools/dartforge-run-ir-before-sdk-data'
dll=root/'target/string-length-native-cache/sdk/6b6831042293752baa4cd97534016ec3/libdfsdk_6b6831042293752b.dylib';old_dll=root/'target/native_cache/sdk/397fd30f2d6130097a559a4fc95d3a5c/libdfsdk_397fd30f2d613009.dylib'
records=[]
for label,executable,ir,library,expected in [('owner-before',before,root/'crates/jit/tests/fixtures/sdk_context_owner.ll',dll,1),('owner-after',runner,root/'crates/jit/tests/fixtures/sdk_context_owner.ll',dll,0),('pruned-after',runner,fixture/'development.ll',dll,0),('old-pruned-after',runner,fixture/'old-development.ll',old_dll,0),('regular-after',runner,fixture/'jit.ll',dll,0)]:
 for stress in [False,True]:
  env=os.environ.copy();env['DARTFORGE_SDK_DLL']=str(library);env.pop('DARTFORGE_GC_STRESS',None)
  if stress:env['DARTFORGE_GC_STRESS']='1'
  name=label+('-stress' if stress else '-normal');command=[str(executable),str(ir)];result=subprocess.run(command,env=env,capture_output=True)
  (out/(name+'.stdout')).write_bytes(result.stdout);(out/(name+'.stderr')).write_bytes(result.stderr)
  correct=result.returncode==expected and (label.startswith('owner-') or result.stdout==b'35294\n')
  records.append(dict(name=name,command=command,exit=result.returncode,expected_exit=expected,correct=correct,sdk_dll=str(library),gc_stress=env.get('DARTFORGE_GC_STRESS')))
  (out/'execution.json').write_text(json.dumps(records,indent=2)+'\n');print(name,result.returncode,correct,flush=True)
  if not correct:raise SystemExit(1)
paths=[runner,before,dll,old_dll,root/'crates/jit/tests/fixtures/sdk_context_owner.ll',fixture/'development.ll',fixture/'old-development.ll',fixture/'jit.ll']
(out/'bindings.json').write_text(json.dumps({str(path):hashlib.sha256(path.read_bytes()).hexdigest() for path in paths},indent=2)+'\n')

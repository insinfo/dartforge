from pathlib import Path
import os,subprocess,json,hashlib,resource,time,shutil
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-pruned-jit');dest=root/'target/test-tools/dartforge-run-ir-pruned-check';tmp=dest.with_suffix('.publish.tmp');shutil.copy2(root/'target/string-length-validation/debug/dartforge-run-ir',tmp);os.replace(tmp,dest)
ir=Path('/tmp/forge-string-length-reuse/development.ll');dll=root/'target/string-length-native-cache/sdk/6b6831042293752baa4cd97534016ec3/libdfsdk_6b6831042293752b.dylib';env=os.environ.copy();env['DARTFORGE_SDK_DLL']=str(dll);env['PATH']='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin:'+env['PATH'];env.pop('DARTFORGE_GC_STRESS',None);rows=[]
def limits():resource.setrlimit(resource.RLIMIT_CORE,(0,0))
for name,runner,stress in [('preserved-runner',root/'target/debug/dartforge-run-ir',False),('current-runner',dest,False),('current-runner-stress',dest,True)]:
 runtime=env.copy()
 if stress:runtime['DARTFORGE_GC_STRESS']='1'
 command=[str(runner),str(ir)];start=time.monotonic();p=subprocess.run(command,cwd=root,env=runtime,capture_output=True,text=True,preexec_fn=limits,timeout=60);row=dict(name=name,command=command,exit=p.returncode,stdout=p.stdout,stderr=p.stderr,seconds=time.monotonic()-start,stress=stress,runner_sha256=hashlib.sha256(runner.read_bytes()).hexdigest(),sdk=str(dll),sdk_sha256=hashlib.sha256(dll.read_bytes()).hexdigest(),ir_sha256=hashlib.sha256(ir.read_bytes()).hexdigest());rows.append(row);(out/'reproduction.json').write_text(json.dumps(rows,indent=2)+'\n');print(name,p.returncode,repr(p.stdout),p.stderr[:500],flush=True)

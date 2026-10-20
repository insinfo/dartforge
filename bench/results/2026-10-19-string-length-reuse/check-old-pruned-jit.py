import os,subprocess,json,hashlib
from pathlib import Path
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-string-length-reuse');compiler=root/'target/test-tools/dartforge-root-policy-fixed';runner=root/'target/debug/dartforge-run-ir'
env=os.environ.copy();env.update(DARTFORGE_SDK_LIB='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib',SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk',MACOSX_DEPLOYMENT_TARGET='15.0');env['PATH']='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin:'+env['PATH'];env.pop('DARTFORGE_NO_STRING_LENGTH_REUSE',None);env.pop('DARTFORGE_GC_STRESS',None)
command=[str(compiler),'compile-native',str(out/'lengths.dart'),'--sdk',env['DARTFORGE_SDK_LIB'],'--emit-ir','-o',str(out/'old-development.ll')]
result=subprocess.run(command,env=env,capture_output=True);(out/'old-ir.stdout').write_bytes(result.stdout);(out/'old-ir.stderr').write_bytes(result.stderr);assert result.returncode==0,result.stderr
loads=subprocess.check_output(['otool','-L',str(out/'old-development')],text=True);name=next(line.strip().split(' (')[0].split('/')[-1] for line in loads.splitlines() if 'libdfsdk_' in line)
candidates=list((root/'target/native_cache/sdk').glob('*/'+name));assert len(candidates)==1,candidates;dll=candidates[0].resolve();env['DARTFORGE_SDK_DLL']=str(dll)
results=[]
for label,command in [('old-aot',[str(out/'old-development')]),('old-pruned-jit',[str(runner),str(out/'old-development.ll')])]:
 result=subprocess.run(command,env=env,capture_output=True);(out/(label+'.stdout')).write_bytes(result.stdout);(out/(label+'.stderr')).write_bytes(result.stderr);results.append(dict(label=label,command=command,exit=result.returncode,stdout=result.stdout.decode(errors='replace')));print(label,result.returncode,flush=True)
record=dict(results=results,sdk_dll=str(dll),sha256={str(path):hashlib.sha256(path.read_bytes()).hexdigest() for path in [compiler,runner,dll,out/'old-development.ll']})
(out/'old-pruned-jit.json').write_text(json.dumps(record,indent=2)+'\n')

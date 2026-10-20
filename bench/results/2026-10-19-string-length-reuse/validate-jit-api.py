import os,subprocess,json,hashlib
from pathlib import Path
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-string-length-reuse')
dll=root/'target/string-length-native-cache/sdk/6b6831042293752baa4cd97534016ec3/libdfsdk_6b6831042293752b.dylib'
runner=root/'target/debug/dartforge-run-ir';emitter=out/'emit-jit';ir=out/'jit.ll'
env=os.environ.copy();env.update(DARTFORGE_SDK_LIB='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib',DARTFORGE_NATIVE_CACHE=str(root/'target/string-length-native-cache'),DARTFORGE_SDK_DLL=str(dll));env['PATH']='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin:'+env['PATH'];env.pop('DARTFORGE_GC_STRESS',None)
records=[]
for name,command,stress in [('jit-api-emit',[str(emitter)],False),('jit-api-normal',[str(runner),str(ir)],False),('jit-api-stress',[str(runner),str(ir)],True)]:
 selected=env.copy()
 if stress:selected['DARTFORGE_GC_STRESS']='1'
 result=subprocess.run(command,env=selected,capture_output=True)
 (out/(name+'.stdout')).write_bytes(result.stdout);(out/(name+'.stderr')).write_bytes(result.stderr)
 matched=result.returncode==0 and (name=='jit-api-emit' or result.stdout==(out/'dart.stdout').read_bytes())
 records.append(dict(name=name,command=command,exit=result.returncode,matches_vm=matched,gc_stress=selected.get('DARTFORGE_GC_STRESS'),sdk_dll=str(dll)))
 (out/'jit-api-results.json').write_text(json.dumps(records,indent=2)+'\n');print(name,result.returncode,matched,flush=True)
 if not matched:raise SystemExit(1)
paths=[dll,dll.parent/'exportados.def',runner,emitter,out/'emit-jit.rs',ir,root/'target/string-length-validation/debug/deps/libdartforge_emit_native-fad3899331e85272.rlib']
(out/'jit-api-bindings.json').write_text(json.dumps({str(path):hashlib.sha256(path.read_bytes()).hexdigest() for path in paths},indent=2)+'\n')

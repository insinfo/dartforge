import json, os, pathlib, subprocess
root=pathlib.Path.cwd(); out=root/'bench/results/2026-10-18-aot-continuation'
env=dict(os.environ,DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang',DARTFORGE_SDK_LIB='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib')
compiler=root/'target/impl-a/release/dartforge'
results=[]
for mode in ['check','tables']:
    for name in ['134_estouro_de_pilha','145_literal_implicit_casts']:
        src=root/f'corpus/native/{name}.dart'; exe=root/f'target/final-{name}-{mode}'
        built=subprocess.run([str(compiler),'compile-native',str(src),'--optimize','--enable-asserts','--exceptions',mode,'-o',str(exe)],env=env,capture_output=True,timeout=300)
        (out/f'final-{name}-{mode}.build.log').write_bytes(built.stdout+built.stderr)
        if built.returncode: raise RuntimeError(built.stderr.decode())
        oracle=subprocess.run(['dart','run','--enable-asserts',str(src)],capture_output=True,timeout=30)
        for stress in [False,True]:
            current=dict(env)
            if stress: current['DARTFORGE_GC_STRESS']='1'
            actual=subprocess.run([str(exe)],capture_output=True,env=current,timeout=30)
            (out/f'final-{name}-{mode}-{stress}.out').write_bytes(actual.stdout)
            result=dict(source=name,mode=mode,stress=stress,equal=actual.stdout==oracle.stdout and actual.returncode==oracle.returncode)
            results.append(result);print(json.dumps(result),flush=True)
            assert result['equal'], (actual.stdout, actual.stderr, oracle.stdout)
(out/'final-smoke.json').write_text(json.dumps(results,indent=2)+'\n')

from pathlib import Path
import subprocess,os,json,time,hashlib,sys
root=next(p for p in Path(__file__).resolve().parents if (p/'Cargo.toml').exists())
out=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else root/'target/finalizador-callback-validacao'
out.mkdir(exist_ok=False)
df=root/'target/release/dartforge.exe'
env=dict(os.environ)
env.update(DARTFORGE_GC_STRESS='1',DARTFORGE_ARC_CONFERIR='1',DARTFORGE_ARC_CICLOS='sempre',DARTFORGE_GC_RASTRO='0',DARTFORGE_HEAP_MAX_MB='256')
env.pop('DARTFORGE_SDK_DLL',None)
rows=[]
def run(etapa,cmd,expected=None,exec_env=None):
    start=time.perf_counter()
    r=subprocess.run([str(x) for x in cmd],cwd=root,env=exec_env or env,capture_output=True,text=True,encoding='utf-8',errors='replace',timeout=600)
    rows.append(dict(etapa=etapa,cmd=[str(x) for x in cmd],codigo=r.returncode,stdout=r.stdout,stderr=r.stderr,segundos=time.perf_counter()-start))
    (out/'resultados.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print(etapa,r.returncode,flush=True)
    assert r.returncode==0,(etapa,r.stderr[-1600:])
    if expected is not None:assert r.stdout.splitlines()==expected,(etapa,r.stdout)
    return r.stdout.splitlines()
(out/'cli.json').write_text(json.dumps(dict(sha256=hashlib.sha256(df.read_bytes()).hexdigest(),fonte='0f6629ad',gc_stress='presente',arc_conferir='1',arc_ciclos='sempre'),indent=2)+'\n',encoding='utf-8')
valid=root/'corpus/nativo/14_finalizadores.dart'
invalid=Path(__file__).with_name('diagnostico.dart')
dart=Path(env['DART_SDK'])/'bin/dart.exe'
oracle_env=dict(env);oracle_env.pop('DARTFORGE_GC_STRESS',None)
expected=run('Dart-validos',[dart,'run',valid],exec_env=oracle_env)
for modo in ['tracing','arc']:
    for nome,fonte,esperado in [('invalidos',invalid,['[true, true]']),('validos',valid,expected)]:
        exe=out/f'{nome}-{modo}.exe'
        run(f'compilar-{nome}-{modo}',[df,'aot',fonte,exe,'--optimize','--memoria',modo])
        run(f'executar-{nome}-{modo}',[exe],esperado)
for nome,fonte,esperado in [('invalidos',invalid,['[true, true]']),('validos',valid,expected)]:
    run(f'JIT-{nome}',[df,'run',fonte],esperado)
print('AOT tracing/ARC e JIT aprovados; caso válido igual ao Dart',flush=True)

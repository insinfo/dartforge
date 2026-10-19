import concurrent.futures, json, os, pathlib, subprocess
root = pathlib.Path.cwd()
out = root / 'bench/results/2026-10-18-aot-continuation'
bin = root / 'target/impl-a/release/dartforge'
env = dict(os.environ, DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang', DARTFORGE_SDK_LIB='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib')
prefixes = ('27_', '57_', '80_', '100_', '101_', '110_', '122_', '127_', '134_', '136_', '137_', '138_', '139_', '141_', '144_', '145_')
files = sorted(p for p in (root/'corpus/native').glob('*.dart') if p.name.startswith(prefixes))
def run(cmd, **kw):
    return subprocess.run([str(v) for v in cmd], stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=180, **kw)
def one(item):
    src, mode = item
    name = src.stem+'.'+mode
    result = {'source':src.name, 'mode':mode}
    try:
        oracle = run(['dart', 'run', '--enable-asserts', src], cwd=src.parent)
        (out/(src.stem+'.vm.out')).write_bytes(oracle.stdout)
        exe = root/'target'/('continuation-'+name)
        built = run([bin, 'compile-native', src, '--optimize', '--enable-asserts', '--exceptions', mode, '-o', exe], env=env)
        (out/(name+'.build.log')).write_bytes(built.stdout+built.stderr)
        if built.returncode:
            result['build']='failed'; return result
        result['build']='passed'; result['runs']=[]
        for stress in (False,True):
            runtime_env = dict(env)
            if stress: runtime_env['DARTFORGE_GC_STRESS']='1'
            actual = run([exe], env=runtime_env, cwd=src.parent)
            tag = '.stress' if stress else ''
            (out/(name+tag+'.out')).write_bytes(actual.stdout)
            (out/(name+tag+'.err')).write_bytes(actual.stderr)
            result['runs'].append({'stress':stress,'exit':actual.returncode,'oracle_exit':oracle.returncode,'equal':actual.stdout==oracle.stdout and actual.returncode==oracle.returncode})
    except subprocess.TimeoutExpired:
        result['timeout']=True
    print(json.dumps(result), flush=True)
    return result
with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
    results=list(pool.map(one,[(src, mode) for src in files for mode in ('check','tables')]))
(out/'targeted-corpus.json').write_text(json.dumps(results,indent=2)+'\n')

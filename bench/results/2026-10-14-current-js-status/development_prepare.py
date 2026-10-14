"""Compile unchanged ordinary kernels under current development emitter and DDC."""
from pathlib import Path
import hashlib,json,os,re,subprocess,sys,time
ROOT=Path.cwd();OUT=ROOT/'target/current-js-status';SDK=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk');CLI=Path(sys.argv[1]).resolve();RUNTIME=ROOT/'runtime/ddc/dart_sdk.js'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
ledger={'compiler':str(CLI),'compiler_sha256':sha(CLI),'runtime_path':str(RUNTIME),'runtime_sha256':sha(RUNTIME),'cases':{},'scope':'Development modules share installed generated Dart 3.6.2 SDK runtime. Cold Node startup separate from warmed kernel samples. No full application readiness claim.'}
for scope in ['correctness','timing']:
    for case in ['typed_data','calls']:
        source=OUT/'correctness'/f'{case}.dart' if scope=='correctness' else ROOT/f'bench/performance/{case}.dart'
        profiles={}
        for profile in ['ForgeDev','DDC']:
            folder=OUT/'development'/scope/case/profile;folder.mkdir(parents=True,exist_ok=True)
            if profile=='ForgeDev':command=[str(CLI),'compile-js',str(source),'-o',str(folder),'--sdk',str(SDK/'lib')]
            else:command=[str(SDK/'bin/dart'),str(SDK/'bin/snapshots/dartdevc.dart.snapshot'),'--modules=es6','-o',str(folder/'program.js'),source.name]
            t=time.monotonic();result=subprocess.run(command,capture_output=True,text=True,cwd=source.parent,timeout=300);elapsed=time.monotonic()-t
            (folder/'compile.stdout').write_text(result.stdout);(folder/'compile.stderr').write_text(result.stderr)
            item={'command':command,'cwd':str(source.parent),'exit':result.returncode,'compile_wall_seconds':elapsed}
            if result.returncode==0:
                if profile=='DDC':
                    artifact=folder/'program.js';text=artifact.read_text().replace("from 'dart_sdk.js'",f"from '{RUNTIME.as_uri()}'");artifact.write_text(text)
                    exported=re.search(r'^export \{\s*([^,}]+)',text,re.M)[1].strip().split(' as ')[-1]
                    (folder/'main.mjs').write_text(f"import {{ {exported} as m }} from './program.js';\nm.main();\n")
                wrapper=folder/'host.mjs';wrapper.write_text("globalThis.self=globalThis;globalThis.window=globalThis;await import('./main.mjs');\n")
                files={str(p.relative_to(folder)):sha(p) for p in folder.rglob('*') if p.is_file() and p.suffix in ['.js','.mjs']}
                item.update(folder=str(folder),host=str(wrapper),artifacts=files,source_sha256=sha(source),emitted_module_bytes_excluding_shared_sdk=sum((folder/f).stat().st_size for f in files if not f.startswith('dart_sdk')),shared_sdk_runtime_bytes=RUNTIME.stat().st_size,size_policy='Own module and wrapper bytes separate from shared SDK runtime; Forge copies the runtime, DDC references the same captured runtime.')
            profiles[profile]=item;print(scope,case,profile,result.returncode,flush=True)
        ledger['cases'][scope+'.'+case]=profiles
assert sha(CLI)==ledger['compiler_sha256'] and sha(RUNTIME)==ledger['runtime_sha256']
oracles=json.loads((OUT/'correctness.json').read_text())
checks={}
for case in ['typed_data','calls']:
    checks[case]={}
    for profile in ['ForgeDev','DDC']:
        item=ledger['cases']['correctness.'+case][profile]
        if item['exit'] != 0:
            checks[case][profile]={'compile_exit':item['exit']};continue
        command=['/Applications/ChatGPT.app/Contents/Resources/cua_node/bin/node',item['host']]
        run=subprocess.run(command,capture_output=True,text=True,timeout=120)
        checks[case][profile]={'command':command,'exit':run.returncode,'stdout':run.stdout,'stderr':run.stderr,'exact_O2':run.returncode==0 and run.stdout==oracles['cases'][case]['runs']['O2']['stdout']}
        print('correctness',case,profile,checks[case][profile]['exact_O2'],flush=True)
ledger['correctness']=checks
(OUT/'development.json').write_text(json.dumps(ledger,indent=2)+'\n')

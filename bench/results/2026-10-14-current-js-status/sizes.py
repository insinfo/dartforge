"""Fresh compile-only size profiles; O4 is an unchecked reference, not parity."""
from pathlib import Path
import gzip,hashlib,json,os,subprocess,time
ROOT=Path.cwd();OUT=ROOT/'target/current-js-status';SDK=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk');COMPILER=OUT/'compiler/dartforge-jsprod'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
historical=json.loads((ROOT/'bench/results/2026-10-14-js-size-profiles/comparison.json').read_text())
cases={n:(Path(v['source']),Path(v['package_config']) if v['package_config'] else None) for n,v in historical['cases'].items() if n in ['limitless_ui','new_sali']}
assert len(cases)==2,cases
cases['hello']=(ROOT/'bench/comparison/hello.dart',None)
input_manifest=ROOT/'target/tmp-js-production-audit/size-reconciliation/input-hashes.json';inputs=json.loads(input_manifest.read_text())
assert all(sha(p)==h for p,h in inputs.items())
results={}
for case,(source,config) in cases.items():
    profiles={}
    for profile in ['forge_checked','dart2js_O2','dart2js_O4']:
        folder=OUT/'sizes'/case/profile;folder.mkdir(parents=True,exist_ok=True);artifact=folder/'program.js'
        cmd=[str(COMPILER),str(source),'--sdk',str(SDK/'lib'),'-o',str(artifact)]+(['--packages',str(config)] if config else []) if profile=='forge_checked' else [str(SDK/'bin/dart'),'compile','js','-'+profile.split('_')[-1],str(source),'-o',str(artifact)]+([f'--packages={config}'] if config else [])
        t=time.monotonic();r=subprocess.run(cmd,capture_output=True,text=True,timeout=300,env=dict(os.environ,TMPDIR=str(OUT)));elapsed=time.monotonic()-t
        (folder/'compile.stdout').write_text(r.stdout);(folder/'compile.stderr').write_text(r.stderr)
        item={'command':cmd,'exit':r.returncode,'compile_wall_seconds':elapsed,'scope':'Single compile invocation; Rust host build recorded separately.'}
        if r.returncode==0:
            data=artifact.read_bytes();item.update(artifact=str(artifact),sha256=sha(artifact),bytes=len(data),gzip9_bytes=len(gzip.compress(data,compresslevel=9,mtime=0)))
        profiles[profile]=item;print(case,profile,r.returncode,item.get('bytes'),flush=True)
    results[case]={'source':str(source),'source_sha256':sha(source),'package_config':str(config) if config else None,'package_config_sha256':sha(config) if config else None,'profiles':profiles}
assert all(sha(p)==h for p,h in inputs.items())
report={'compiler_sha256':sha(COMPILER),'source_input_manifest_reference':str(input_manifest.relative_to(ROOT)),'source_input_manifest_sha256':sha(input_manifest),'source_manifest_durable_reference':'bench/results/2026-10-14-js-literal-fragments-rejected/input-hashes.json','all_inputs_stable':True,'scope':'Compile-only size profiles for frozen captured apps, not fresh browser startup evidence. O2 checked reference; O4 omits checks/trusts primitives and has differing semantics.','cases':results}
(OUT/'sizes.json').write_text(json.dumps(report,indent=2)+'\n')

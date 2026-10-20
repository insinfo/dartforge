from pathlib import Path
import os,subprocess,json,hashlib,time
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=root/'target/root-policy-validation';out.mkdir(exist_ok=True)
cache=out/'native-cache';assert not cache.exists(),'Integration requires an empty cache; do not overwrite an earlier run'
compiler=root/'target/test-tools/dartforge-root-policy-fixed';source=root/'target/dense-dominator-validation/branches.dart';prefix='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64'
env=os.environ.copy();env.update({'DARTFORGE_NATIVE_CACHE':str(cache),'SDKROOT':'/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk','MACOSX_DEPLOYMENT_TARGET':'15.0'});env['PATH']=prefix+'/bin:/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin:'+env['PATH'];env.pop('DARTFORGE_PACK_RELR',None);env.pop('DARTFORGE_ROOTS_IN_CALLEE',None);env.pop('DARTFORGE_OPTIMIZE_HIR',None);env.update({'DARTFORGE_MEMORY':'tracing','DARTFORGE_ROOTS':'shadow'})
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def inventory():
 return {p.name:{'files':{f.name:sha(f) for f in sorted(p.iterdir()) if f.suffix in ['.o','.poda']}} for p in (cache/'sdk').iterdir() if p.is_dir() and (p/'pronto').is_file()}
rows=[];snapshots=[]
for label,mode in [('default',None),('enabled','1'),('disabled','0')]:
 current=env.copy()
 if mode is not None:current['DARTFORGE_ROOTS_IN_CALLEE']=mode
 command=[str(compiler),'compile-native',str(source),'--emit-ir','--digest','--timings','--sdk','/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib','-o',str(out/(label+'.ll'))]
 start=time.monotonic();result=subprocess.run(command,cwd=root,env=current,capture_output=True);elapsed=time.monotonic()-start
 (out/(label+'.stdout')).write_bytes(result.stdout);(out/(label+'.stderr')).write_bytes(result.stderr)
 assert result.returncode==0,result.stderr.decode()
 state=inventory();snapshots.append(state)
 rows.append({'label':label,'mode':mode,'command':command,'exit':result.returncode,'seconds':elapsed,'compiler_sha256':sha(compiler),'source_sha256':sha(source),'ir_sha256':sha(out/(label+'.ll')),'sdk':state,'sdk_ids':{f.name:sha(f) for f in sorted((cache/'sdk_ids').glob('*.tsv'))}})
 (out/'runs.json').write_text(json.dumps(rows,indent=2)+'\n');print(label,list(state),flush=True)
 assert len(state)==(1 if label=='default' else 2)
assert snapshots[0].keys() < snapshots[1].keys()
assert snapshots[1]==snapshots[2]
assert all(snapshots[1][k]==v for k,v in snapshots[0].items())
assert rows[0]['sdk_ids']==rows[1]['sdk_ids']==rows[2]['sdk_ids']
assert (out/'default.ll').read_bytes()==(out/'disabled.ll').read_bytes()
assert (out/'default.ll').read_bytes()!=(out/'enabled.ll').read_bytes()
(out/'verification.json').write_text(json.dumps({'default_and_enabled_sdk_directories_distinct':True,'disabled_reuses_default_artifacts_unchanged':True,'enabled_does_not_replace_default_artifacts':True,'source_class_catalog_unchanged':True,'default_equals_disabled_ir':True,'enabled_ir_differs':True,'sdk_artifact_counts':[len(s) for s in snapshots]},indent=2)+'\n')
print('Actual SDK cache toggles separate callee rooting strategies and preserve source class IDs.')

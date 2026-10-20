from pathlib import Path
import os,subprocess,json,time,hashlib,resource,sys
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-root-bit-iteration/benchmark');out.mkdir(exist_ok=True)
artifacts=root/'target/root-bit-iteration-benchmark';artifacts.mkdir(exist_ok=True)
prefix='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64';source=Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui/examples/file_explorer/bin/file_explorer.dart')
compilers={'before':root/'target/test-tools/dartforge-byte-literal-pruning','after':root/'target/test-tools/dartforge-root-bit-iteration'}
env=os.environ.copy();env.update(DARTFORGE_LLVM_DIR=prefix,LLVM_SYS_221_PREFIX=prefix,SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk',MACOSX_DEPLOYMENT_TARGET='15.0',DARTFORGE_SDK_LIB='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib',DARTFORGE_NATIVE_CACHE=str(root/'target/rpo-native-cache'),DARTFORGE_GENERATOR='clang');env['PATH']=prefix+'/bin:'+env['PATH']
for key in ['DARTFORGE_CLOSED_CLASS_REACHABILITY','DARTFORGE_NO_PRUNING','DARTFORGE_NO_TABLE_PRUNING','DARTFORGE_GC_STRESS']:env.pop(key,None)
rows=[];expected_ir=None;expected_sources=None
for index,(variant,warmup) in enumerate([('before',True),('after',True)]+[(v,False) for v in ['before','after','after','before','before','after','after','before']]):
 compiler=compilers[variant];label=f'{index:02d}-{variant}';ir=artifacts/(label+'.ll');command=[str(compiler),'compile-native',str(source),'--sdk',env['DARTFORGE_SDK_LIB'],'--optimize','--emit-ir','--timings','-o',str(ir)]
 started=time.monotonic();print('Starting',label,'warmup' if warmup else 'measurement',flush=True)
 with (out/(label+'.log')).open('w') as log:
  p=subprocess.Popen(command,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
  (out/'state.json').write_text(json.dumps(dict(status='running',label=label,pid=p.pid,command=command),indent=2)+'\n')
  _,status,usage=os.wait4(p.pid,0);p.returncode=os.waitstatus_to_exitcode(status)
 elapsed=time.monotonic()-started
 if p.returncode!=0:
  rows.append(dict(label=label,variant=variant,warmup=warmup,command=command,exit=p.returncode,wall_seconds=elapsed));(out/'runs.json').write_text(json.dumps(rows,indent=2)+'\n');(out/'state.json').write_text(json.dumps(dict(status='failed',label=label,exit=p.returncode),indent=2)+'\n');raise RuntimeError((label,p.returncode))
 contract=json.loads(ir.with_suffix(ir.suffix+'.contracts.json').read_text());sources=sorted((r['uri'],r['blake3']) for r in contract['sources']);digest=hashlib.sha256(ir.read_bytes()).hexdigest()
 if expected_ir is None:
  expected_ir=digest;expected_sources=sources;(out/'sources.json').write_text(json.dumps(sources,indent=2)+'\n')
 assert digest==expected_ir,(label,'IR changed');assert sources==expected_sources,(label,'source inputs changed')
 rows.append(dict(label=label,variant=variant,warmup=warmup,command=command,exit=p.returncode,wall_seconds=elapsed,user_seconds=usage.ru_utime,system_seconds=usage.ru_stime,max_rss_bytes=usage.ru_maxrss,ir_bytes=ir.stat().st_size,ir_sha256=digest,source_count=len(sources),compiler_sha256=hashlib.sha256(compiler.read_bytes()).hexdigest()))
 (out/'runs.json').write_text(json.dumps(rows,indent=2)+'\n');print('Finished',label,elapsed,flush=True)
(out/'sources.json').write_text(json.dumps(expected_sources,indent=2)+'\n');(out/'state.json').write_text(json.dumps(dict(status='finished',runs=len(rows)),indent=2)+'\n')

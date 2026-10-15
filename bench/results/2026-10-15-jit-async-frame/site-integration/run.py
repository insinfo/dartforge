"""Run serialized compiler/runtime gates with source and log provenance."""
import hashlib,json,os,subprocess,time
from pathlib import Path
root=Path.cwd();out=Path(__file__).resolve().parent
assert not (out/'validation.json').exists(), 'Choose a fresh archive for a retry'
def snapshot():
 return {str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest()
  for parent in ['crates','native_sdk'] for p in sorted((root/parent).rglob('*'))
  if p.is_file() and (p.suffix in {'.rs','.dart','.toml','.tsv','.json'} or p.name=='Cargo.lock')}
steps=[('native-frames',['cargo','test','--locked','-j2','-p','dartforge-emit-native','--lib','frame_','--','--test-threads=2']),
 ('jit-unit',['cargo','test','--locked','-j2','-p','dartforge-jit','--lib','--','--include-ignored','--test-threads=2']),
 ('suspended-frame',['cargo','test','--locked','-j2','-p','dartforge-jit','--test','async_frame_reload','--','--include-ignored','--test-threads=1'])]
before=snapshot();(out/'source.json').write_text(json.dumps(before,indent=2)+'\n')
report={'started':time.time(),'environment':{k:os.environ.get(k) for k in ['CARGO_TARGET_DIR','TMPDIR','LLVM_SYS_221_PREFIX','DARTFORGE_TEST_SDK_LIB','RUSTFLAGS']},'steps':[]}
for label,command in steps:
 started=time.time();log=out/(label+'.log')
 with log.open('wb') as f:result=subprocess.run(command,stdout=f,stderr=subprocess.STDOUT)
 stable=before==snapshot()
 report['steps'].append({'label':label,'command':command,'exit_code':result.returncode,'seconds':time.time()-started,'source_unchanged':stable,'log_sha256':hashlib.sha256(log.read_bytes()).hexdigest()})
 (out/'validation.json').write_text(json.dumps(report,indent=2)+'\n')
 print(json.dumps(report['steps'][-1]),flush=True)
 if result.returncode or not stable:raise SystemExit(result.returncode or 1)

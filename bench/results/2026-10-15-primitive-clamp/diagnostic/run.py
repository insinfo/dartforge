"""Record focused compiler validation without altering failed evidence."""
from pathlib import Path
import hashlib,json,subprocess,os,time
root=Path.cwd(); archive=Path(__file__).resolve().parent
paths=[p for p in (root/'crates/emit_native').rglob('*') if p.is_file() and p.suffix in ['.rs','.toml']] + list((root/'native_sdk').rglob('*.dart'))
def snap(): return {str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}
command=['cargo','test','--locked','-j2','-p','dartforge-emit-native','--lib','actual_literal_clamp','--','--test-threads=2']
before=snap(); report={'command':command,'source_before':before,'started':time.time(),'environment':{k:os.environ.get(k) for k in ['CARGO_TARGET_DIR','TMPDIR','DARTFORGE_SDK_LIB','LLVM_SYS_221_PREFIX']}}
assert not (archive/'validation.json').exists()
(archive/'validation.json').write_text(json.dumps(report,indent=2)+'\n')
with (archive/'tests.log').open('wb') as log:
 result=subprocess.run(command,stdout=log,stderr=subprocess.STDOUT)
report.update(exit_code=result.returncode,source_unchanged=before==snap(),ended=time.time(),log_sha256=hashlib.sha256((archive/'tests.log').read_bytes()).hexdigest())
(archive/'validation.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({k:v for k,v in report.items() if k!='source_before'}),flush=True)
raise SystemExit(result.returncode)

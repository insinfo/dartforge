#!/usr/bin/env python3
"""Run the real self-contained production test against an isolated SDK cache."""
import hashlib,json,os,subprocess,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4]
OUT=Path(__file__).resolve().parent
CACHE=ROOT/'target/allocation-closure-hardening-cache'
def snapshot():
    result={}
    for directory in ('crates/emit_native','crates/runtime','crates/types/src','crates/elements/src','crates/frontend/src','native_sdk'):
        for file in sorted((ROOT/directory).rglob('*')):
            if file.is_file() and file.suffix in ('.rs','.dart','.tsv','.toml'):
                result[str(file.relative_to(ROOT))]=hashlib.sha256(file.read_bytes()).hexdigest()
    return result
before=snapshot();(OUT/'source.json').write_text(json.dumps(before,indent=2)+'\n')
command=['cargo','test','--locked','-j2','-p','dartforge-emit-native','--lib','allocation_inventory','--','--test-threads=2']
report={'command':command,'environment':{k:os.environ.get(k) for k in ['CARGO_TARGET_DIR','DARTFORGE_NATIVE_CACHE','DARTFORGE_TEST_SDK_LIB','DARTFORGE_KEEP_IR','LLVM_SYS_221_PREFIX','DARTFORGE_CLANG','TMPDIR','RUSTFLAGS','LIBRARY_PATH']},'cache_initial_files':[str(p.relative_to(CACHE)) for p in CACHE.rglob('*') if p.is_file()],'started':time.time()}
assert not report['cache_initial_files'],'Fresh cache is required; do not overwrite earlier evidence.'
(OUT/'started.json').write_text(json.dumps(report,indent=2)+'\n')
with (OUT/'tests.log').open('w') as log:
    result=subprocess.run(command,cwd=ROOT,stdout=log,stderr=subprocess.STDOUT)
report.update(exit_code=result.returncode,seconds=time.time()-report['started'],source_unchanged=before==snapshot(),log_sha256=hashlib.sha256((OUT/'tests.log').read_bytes()).hexdigest(),scope='Focused allocation inventory regressions after raw-memory hardening; no performance timing.')
(OUT/'validation.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({k:report[k] for k in ['exit_code','seconds','source_unchanged','log_sha256']},indent=2),flush=True)
raise SystemExit(result.returncode or (0 if report['source_unchanged'] else 3))

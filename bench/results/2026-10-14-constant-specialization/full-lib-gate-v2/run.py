from pathlib import Path
import subprocess,hashlib,json,time,os
root=Path.cwd();base=root/'bench/results/2026-10-14-constant-specialization/full-lib-gate-v2'
def snap():
 names=subprocess.check_output(['rg','--files','crates','native_sdk','packages'],text=True).splitlines()+['Cargo.toml','Cargo.lock','rust-toolchain.toml']
 return {p:hashlib.sha256((root/p).read_bytes()).hexdigest() for p in sorted(set(names)) if (root/p).is_file() and ((root/p).suffix in ['.rs','.dart','.toml','.tsv','.json'] or p=='Cargo.lock')}
command=['cargo','test','--locked','-j2','-p','dartforge-emit-native','--lib','--','--include-ignored','--test-threads=2'];before=snap();start=time.time();m={'command':command,'cwd':str(root),'git_head':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'branch':subprocess.check_output(['git','branch','--show-current'],text=True).strip(),'environment':{k:os.environ.get(k) for k in ['CARGO_HOME','RUSTUP_HOME','PUB_CACHE','CARGO_TARGET_DIR','TMPDIR','DARTFORGE_CLANG','LLVM_SYS_221_PREFIX','DARTFORGE_SDK_LIB']},'source_before':before,'started_unix':start};(base/'validation.json').write_text(json.dumps(m,indent=2)+'\n')
with (base/'full-native-lib.log').open('wb') as log:
 p=subprocess.Popen(command,stdout=log,stderr=subprocess.STDOUT);m['cargo_pid']=p.pid;(base/'validation.json').write_text(json.dumps(m,indent=2)+'\n');print('CARGO_PID='+str(p.pid),flush=True);m['exit_code']=p.wait()
after=snap();m.update({'ended_unix':time.time(),'source_unchanged':before==after,'source_after':after,'changed_sources':sorted(p for p in set(before)|set(after) if before.get(p)!=after.get(p)),'log_sha256':hashlib.sha256((base/'full-native-lib.log').read_bytes()).hexdigest()});(base/'validation.json').write_text(json.dumps(m,indent=2)+'\n');print('EXIT='+str(m['exit_code'])+' SOURCES_UNCHANGED='+str(m['source_unchanged']),flush=True);print('\n'.join((base/'full-native-lib.log').read_text().splitlines()[-8:]),flush=True);raise SystemExit(m['exit_code'])

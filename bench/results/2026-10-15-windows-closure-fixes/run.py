import hashlib,json,os,subprocess,time
from pathlib import Path
root=Path.cwd();out=root/'target/release-ci-audit/windows-closure-fixes'
def snapshot():
 files=list((root/'crates/emit_native').rglob('*.rs'))+[root/'crates/emit_native/Cargo.toml',root/'Cargo.lock']
 return {str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(files)}
before=snapshot();(out/'source.json').write_text(json.dumps(before,indent=2)+'\n')
commands=[['cargo','test','--locked','-j2','-p','dartforge-emit-native','--lib',f,'--','--test-threads=2','--nocapture'] for f in ['selected_source_cache_tests','real_package_creation_preserves_generic_timeout_and_native_signatures','versioned_package_body_is_native_but_arbitrary_pragmas_are_not']]
report={'commands':commands,'source':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'environment':{k:os.environ.get(k) for k in ['CARGO_TARGET_DIR','DARTFORGE_TEST_SDK_LIB','DARTFORGE_SDK_LIB','LLVM_SYS_221_PREFIX','RUSTFLAGS','TMPDIR']},'runs':[]}
for index,cmd in enumerate(commands):
 start=time.time()
 with (out/f'tests-{index}.log').open('w') as log:r=subprocess.run(cmd,cwd=root,stdout=log,stderr=subprocess.STDOUT)
 report['runs'].append({'filter':cmd[7],'exit_code':r.returncode,'seconds':time.time()-start,'log':f'tests-{index}.log'})
 if r.returncode:break
report['source_unchanged']=before==snapshot();report['passed']=len(report['runs'])==3 and all(r['exit_code']==0 for r in report['runs']) and report['source_unchanged']
(out/'validation.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2),flush=True)
raise SystemExit(0 if report['passed'] else 1)

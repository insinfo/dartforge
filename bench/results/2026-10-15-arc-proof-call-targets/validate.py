"""Run the existing affected ARC proof matrices and exact CI foreign profiles."""
from pathlib import Path
import hashlib, importlib.util, json, os, resource, shutil, subprocess, sys

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent / 'validation'
OUT.mkdir(exist_ok=False)
ENV = dict(os.environ, CARGO_BUILD_JOBS='2')
NAMES = ['arc_keepalive_pending', 'arc_field_allocation', 'arc_field_keepalive',
         'arc_list_storage', 'arc_local_fields', 'arc_typed_scalar_return',
         'arc_loop_cleanup', 'arc_foreign_unwind']
SCRIPT = ROOT / 'scripts/prove-arc-aot.py'
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
files = {ROOT/'Cargo.toml', ROOT/'Cargo.lock', ROOT/'rust-toolchain.toml', SCRIPT}
for crate in ['emit_native', 'runtime', 'llvm']:
    directory = ROOT/'crates'/crate
    files.update(p for p in directory.rglob('*') if p.is_file() and (p.suffix in ['.rs','.toml','.c','.h','.cpp','.ll'] or p.name=='build.rs'))
def snapshot(): return {str(p.relative_to(ROOT)):sha(p) for p in sorted(files)}
before = snapshot()
(OUT/'source-sha256.json').write_text(json.dumps(before,indent=2)+'\n')
provenance = {'head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
              'env':{k:ENV.get(k) for k in ['CARGO_BUILD_JOBS','CARGO_TARGET_DIR','TMPDIR','LLVM_SYS_221_PREFIX','DARTFORGE_CLANG','DARTFORGE_SDK_LIB','DARTFORGE_EXCEPTIONS','RUSTFLAGS']},
              'driver_sha256':sha(__file__),
              'rustc':subprocess.check_output(['rustc','--version','--verbose'],env=ENV,text=True)}
(OUT/'provenance.json').write_text(json.dumps(provenance,indent=2)+'\n')
report = {'stages':[], 'scope':'Existing proof script ARC/tracing O0/O2 matrices, plus exact CI foreign-unwind profile/level/mode/path loops. No benchmark/timing claim.'}
resource.setrlimit(resource.RLIMIT_CORE,(0,0))

def run(command, log, env=ENV):
    record = {'command':[str(x) for x in command], 'log':log.name}
    report['stages'].append(record)
    with log.open('wb') as output:
        p = subprocess.run(command,cwd=ROOT,env=env,stdout=output,stderr=subprocess.STDOUT)
    record['exit_code']=p.returncode
    record['log_sha256']=sha(log)
    if p.returncode: raise RuntimeError(f'First failure: {log}, exit {p.returncode}')
    return record

def fresh(directory):
    if directory.exists():
        backup=directory.with_name(directory.name+'-before-arc-symbol-fix-'+str(os.getpid()))
        directory.rename(backup)
    directory.mkdir()

def archive(directory,label):
    # Text proof artifacts only; executables/object files stay on the SSD target.
    destination=OUT/label;destination.mkdir(exist_ok=True)
    import gzip
    for p in directory.iterdir():
        if not p.is_file(): continue
        if p.suffix in ['.ll','.cpp']:
            (destination/(p.name+'.gz')).write_bytes(gzip.compress(p.read_bytes(),mtime=0))
        elif p.suffix in ['.json','.stdout','.stderr','.exit'] or p.name.endswith('.build.log'):
            shutil.copy2(p,destination/p.name)

try:
    command=['cargo','test','--locked','--release','-p','dartforge-emit-native','--features','embedded-llvm']
    for name in NAMES: command+=['--example',name]
    command+=['--','--include-ignored','--nocapture','--test-threads=1']
    run(command,OUT/'example-units.log')
    print('All affected example units passed',flush=True)
    spec=importlib.util.spec_from_file_location('existing_arc_proofs',SCRIPT)
    proofs=importlib.util.module_from_spec(spec);spec.loader.exec_module(proofs)
    for name in NAMES[:-1]:
        directory_name,_,cases=proofs.PROOFS[name]
        directory=ROOT/'target'/directory_name;fresh(directory);current_label=name
        try:
            run([sys.executable,str(SCRIPT),name,'--features','embedded-llvm'],OUT/(name+'.log'))
        finally:
            archive(directory,name)
        evidence=json.loads((directory/'evidence.json').read_text())
        assert evidence['result']=='success' and len(evidence['runs'])==4*len(cases)
        record=report['stages'][-1];record['proof_runs']=len(evidence['runs']);record['positive_runs']=sum(r['control'] is None for r in evidence['runs']);record['negative_ownership_traps']=sum(r['control'] is not None for r in evidence['runs'])
        print(name,record['proof_runs'],'actual AOT proofs passed',flush=True)
    directory=ROOT/'target/proof-foreign-unwind';fresh(directory);current_label='arc_foreign_unwind'
    expected={'cpp':b'foreign-cleanup-ok\n','forced':b'forced-cleanup-ok\n','forced_dart':b'forced-cleanup-ok\n'}
    foreign=[]
    for profile in ['simple','frames','resume']:
        for level in [0,2]:
            exe=directory/f'proof-{profile}-O{level}'
            command=['cargo','run','--locked','--release','-p','dartforge-emit-native','--features','embedded-llvm','--example','arc_foreign_unwind','--',str(exe),str(level),profile]
            run(command,directory/(exe.name+'.build.log'),dict(ENV,DARTFORGE_EFFECTS='check'))
            for mode in ['arc','tracing']:
                for path in ['cpp','forced','forced_dart']:
                    stdout=directory/f'{exe.name}-{mode}-{path}.stdout';stderr=stdout.with_suffix('.stderr')
                    with stdout.open('wb') as out,stderr.open('wb') as err:
                        p=subprocess.run([str(exe),mode,path],cwd=ROOT,env=dict(ENV,DARTFORGE_ARC_CHECK='1',DARTFORGE_ARC_NURSERY='0',DARTFORGE_GC_STRESS='1'),stdout=out,stderr=err)
                    exact=p.returncode==0 and stdout.read_bytes()==expected[path]
                    foreign.append({'profile':profile,'level':level,'mode':mode,'path':path,'exit_code':p.returncode,'exact':exact,'stdout_sha256':sha(stdout),'stderr_sha256':sha(stderr),'binary_sha256':sha(exe)})
                    if not exact: raise RuntimeError(f'Foreign proof failed: {stdout}')
            print('Foreign',profile,level,'all6 executions passed',flush=True)
    (directory/'evidence.json').write_text(json.dumps({'runs':foreign,'result':'success'},indent=2)+'\n')
    archive(directory,'arc_foreign_unwind')
    report['foreign_runs']=len(foreign)
    report['result']='success'
except Exception as error:
    report['result']='failure';report['error']=str(error)
    if 'directory' in locals() and 'current_label' in locals():
        if current_label=='arc_foreign_unwind':
            (directory/'evidence.json').write_text(json.dumps({'result':'failure','error':str(error),'runs':locals().get('foreign',[])},indent=2)+'\n')
        archive(directory,current_label)
    print(str(error),flush=True)
    raise
finally:
    report['sources_unchanged']=before==snapshot()
    (OUT/'result.json').write_text(json.dumps(report,indent=2)+'\n')
    if not report['sources_unchanged']: raise RuntimeError('Source changed during validation')

"""Compare ordinary codec/cache behavior with exact SDK VM oracle; not a benchmark."""
from pathlib import Path
import hashlib,json,os,subprocess,sys
ROOT=Path.cwd();CLI=Path(sys.argv[1]).resolve();OUT=ROOT/'target/canonical-cache-validation';SDK=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk');CLANG=Path('/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang');SOURCE=ROOT/'crates/emit_native/tests/fixtures/canonical_cache/main.dart'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert sha(CLI)==sys.argv[2]
OUT.mkdir(parents=True,exist_ok=True)
env=dict(os.environ,TMPDIR=str(OUT),DARTFORGE_CLANG=str(CLANG),LLVM_SYS_221_PREFIX=str(CLANG.parent.parent),DARTFORGE_SDK_LIB=str(SDK/'lib'),DARTFORGE_NO_PHYSICAL_GETTER_DEVIRTUALIZATION='1',DARTFORGE_NO_ENUM_PATTERN_IDENTITY='1',DARTFORGE_KEEP_IR='1',DARTFORGE_LINK_MAP='1')
for key in ['DARTFORGE_NO_CANONICAL_CACHE_PROVENANCE','DARTFORGE_NO_PRE_PRUNE_CFG','DARTFORGE_NO_PLATFORM_SPECIALIZATION','DARTFORGE_ARC_PREPARED','DARTFORGE_ARC_PREPARE','DARTFORGE_ARC_CHECK','DARTFORGE_EFFECTS','DARTFORGE_GC_STRESS']:
    env.pop(key,None)
command=[str(SDK/'bin/dart'),'run',str(SOURCE)]
vm=subprocess.run(command,capture_output=True,text=True,timeout=30)
(OUT/'vm.stdout').write_text(vm.stdout);(OUT/'vm.stderr').write_text(vm.stderr)
assert vm.returncode==0 and not vm.stderr,vm
report={'compiler':str(CLI),'compiler_sha256':sha(CLI),'source':str(SOURCE),'source_sha256':sha(SOURCE),'SDK_binary_sha256':sha(SDK/'bin/dart'),'SDK_vm_command':command,'VM_stdout':vm.stdout,'common_environment':{'DARTFORGE_NO_PHYSICAL_GETTER_DEVIRTUALIZATION':'1','DARTFORGE_NO_ENUM_PATTERN_IDENTITY':'1'},'enum_setting':'DARTFORGE_NO_ENUM_PATTERN_IDENTITY=1 in both profiles.','runs':{},'scope':'Actual CLI correctness only. No timing, startup or GUI claims.'}
for profile in ['off','on']:
    folder=OUT/profile;folder.mkdir(exist_ok=True);binary=folder/'program';e=dict(env,DARTFORGE_WHY=str(folder/'retention.tsv'))
    if profile=='off':e['DARTFORGE_NO_CANONICAL_CACHE_PROVENANCE']='1'
    cmd=[str(CLI),'compile-native',str(SOURCE),'--sdk',str(SDK/'lib'),'--optimize','--memory=tracing','--exceptions=check','--roots=shadow','--stack-trace=none','-o',str(binary)]
    compiled=subprocess.run(cmd,capture_output=True,text=True,env=e,timeout=600)
    (folder/'compile.stdout').write_text(compiled.stdout);(folder/'compile.stderr').write_text(compiled.stderr)
    item={'command':cmd,'compile_exit':compiled.returncode,'opt_out':e.get('DARTFORGE_NO_CANONICAL_CACHE_PROVENANCE')}
    if compiled.returncode==0:
        item.update(binary_sha256=sha(binary),binary_bytes=binary.stat().st_size)
        for stress in [False,True]:
            runtime=e.copy()
            if stress:runtime['DARTFORGE_GC_STRESS']='1'
            executed=subprocess.run([str(binary)],capture_output=True,text=True,env=runtime,timeout=60)
            name='stress' if stress else 'normal';(folder/(name+'.stdout')).write_text(executed.stdout);(folder/(name+'.stderr')).write_text(executed.stderr)
            item[name]={'exit':executed.returncode,'stdout':executed.stdout,'stderr':executed.stderr,'exact':executed.returncode==0 and executed.stdout==vm.stdout and not executed.stderr}
    report['runs'][profile]=item;(OUT/'validation.json').write_text(json.dumps(report,indent=2)+'\n');print(profile,item,flush=True)
    assert item.get('normal',{}).get('exact') and item.get('stress',{}).get('exact'),item
assert sha(CLI)==report['compiler_sha256'] and sha(SOURCE)==report['source_sha256']
report['inputs_stable']=True;(OUT/'validation.json').write_text(json.dumps(report,indent=2)+'\n')

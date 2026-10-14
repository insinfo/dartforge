"""Actual CLI checks for explicit SDK selection and early constant CFG cleanup."""
from pathlib import Path
import hashlib, json, os, subprocess, sys
ROOT=Path.cwd(); CLI=Path(sys.argv[1]).resolve(); OUT=ROOT/'target/sdk-pre-prune-validation'
SDK=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk')
CLANG=Path('/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang')
WRAPPER=ROOT/'target/platform-specialization/modified-sdk/lib'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
OUT.mkdir(parents=True,exist_ok=True)
env=dict(os.environ,TMPDIR=str(OUT),DARTFORGE_CLANG=str(CLANG),LLVM_SYS_221_PREFIX=str(CLANG.parent.parent),DARTFORGE_SDK_LIB=str(SDK/'lib'))
for key in ['DARTFORGE_NO_PLATFORM_SPECIALIZATION','DARTFORGE_NO_PRE_PRUNE_CFG','DARTFORGE_ARC_PREPARED','DARTFORGE_ARC_CHECK','DARTFORGE_EFFECTS','DARTFORGE_GC_STRESS']:
    env.pop(key,None)
sources=[ROOT/'crates/emit_native/tests/fixtures/platform_flags/main.dart',ROOT/'crates/emit_native/tests/fixtures/pre_prune_cfg/main.dart']
tracked=sources+list(sources[0].parent.glob('reexport.dart'))+[SDK/'lib/io/platform_impl.dart',WRAPPER/'io/platform_impl.dart']
report={'compiler':str(CLI),'compiler_sha256':sha(CLI),'inputs':{str(p):sha(p) for p in tracked},'runs':{},'scope':'Actual CLI correctness only. Installed VM is the original SDK oracle; copied SDK expected wrapper effect is checked explicitly. No performance or GUI claims.'}
for source,label,sdk in [(sources[0],'selected-wrapper',WRAPPER),(sources[1],'cfg',SDK/'lib')]:
    vm=subprocess.run([str(SDK/'bin/dart'),'run',str(source)],capture_output=True,text=True,timeout=30)
    assert vm.returncode==0 and not vm.stderr,vm
    expected=('platform wrapper\n' if label=='selected-wrapper' else '')+vm.stdout
    for variant in (['cold','warm'] if label=='selected-wrapper' else ['off','on']):
        folder=OUT/label/variant;folder.mkdir(parents=True,exist_ok=True)
        binary=folder/'program';e=env.copy()
        if variant=='off':e['DARTFORGE_NO_PRE_PRUNE_CFG']='1'
        cmd=[str(CLI),'compile-native',str(source),'--sdk',str(sdk),'--optimize','--memory=tracing','--exceptions=check','--roots=shadow','--stack-trace=none','-o',str(binary)]
        compiled=subprocess.run(cmd,capture_output=True,text=True,env=e,timeout=300)
        (folder/'compile.stdout').write_text(compiled.stdout);(folder/'compile.stderr').write_text(compiled.stderr)
        item={'command':cmd,'compile_exit':compiled.returncode,'discovered_env_SDK':e['DARTFORGE_SDK_LIB'],'selected_SDK':str(sdk),'opt_out':e.get('DARTFORGE_NO_PRE_PRUNE_CFG')}
        if compiled.returncode==0:
            ran=subprocess.run([str(binary)],capture_output=True,text=True,env=e,timeout=30)
            (folder/'run.stdout').write_text(ran.stdout);(folder/'run.stderr').write_text(ran.stderr)
            item.update(binary_sha256=sha(binary),exit=ran.returncode,stdout=ran.stdout,stderr=ran.stderr,expected=expected,exact=ran.returncode==0 and ran.stdout==expected and not ran.stderr)
        report['runs'][label+'.'+variant]=item
        (OUT/'validation.json').write_text(json.dumps(report,indent=2)+'\n')
        print(label,variant,item,flush=True)
        assert item.get('exact'),item
assert sha(CLI)==report['compiler_sha256'] and all(sha(p)==h for p,h in report['inputs'].items())
report['inputs_stable']=True
(OUT/'validation.json').write_text(json.dumps(report,indent=2)+'\n')

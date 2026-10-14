"""Build paired tracing benchmark artifacts; execute only in a granted quiet window."""
from pathlib import Path
import hashlib,json,os,subprocess,sys
ROOT=Path.cwd();CLI=Path(sys.argv[1]).resolve();OUT=ROOT/'target/sdk-pre-prune-cpu';SOURCE=ROOT/'bench/comparison/bench.dart'
SDK=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk');CLANG=Path('/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang')
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert sha(CLI)==sys.argv[2]
OUT.mkdir(parents=True,exist_ok=True)
env=dict(os.environ,TMPDIR=str(OUT),DARTFORGE_CLANG=str(CLANG),LLVM_SYS_221_PREFIX=str(CLANG.parent.parent),DARTFORGE_SDK_LIB=str(SDK/'lib'))
for key in ['DARTFORGE_NO_PRE_PRUNE_CFG','DARTFORGE_ARC_PREPARED','DARTFORGE_ARC_CHECK','DARTFORGE_EFFECTS','DARTFORGE_GC_STRESS','DARTFORGE_NO_PLATFORM_SPECIALIZATION']:
    env.pop(key,None)
report={'compiler_sha256':sha(CLI),'source_sha256':sha(SOURCE),'profiles':{},'scope':'Compile-only preparation. No benchmark executions/timings.'}
for profile in ['off','on']:
    folder=OUT/profile;folder.mkdir(exist_ok=True);binary=folder/'program';e=env.copy()
    if profile=='off':e['DARTFORGE_NO_PRE_PRUNE_CFG']='1'
    command=[str(CLI),'compile-native',str(SOURCE),'--sdk',str(SDK/'lib'),'--optimize','--memory=tracing','--exceptions=check','--roots=shadow','--stack-trace=none','-o',str(binary)]
    result=subprocess.run(command,capture_output=True,text=True,env=e,timeout=600)
    (folder/'compile.stdout').write_text(result.stdout);(folder/'compile.stderr').write_text(result.stderr)
    report['profiles'][profile]={'command':command,'exit':result.returncode,'opt_out':e.get('DARTFORGE_NO_PRE_PRUNE_CFG'),'binary':str(binary),'binary_sha256':sha(binary) if result.returncode==0 else None}
    (OUT/'artifacts.json').write_text(json.dumps(report,indent=2)+'\n')
    assert result.returncode==0,result.stderr
assert sha(CLI)==report['compiler_sha256'] and sha(SOURCE)==report['source_sha256']

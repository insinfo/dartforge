"""Matched native OFF/ON correctness; no performance measurements."""
from pathlib import Path
import hashlib,json,os,subprocess,sys
ROOT=Path.cwd();CLI=Path(sys.argv[1]).resolve();OUT=ROOT/'target/selector-getter-validation';SDK=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk');CLANG=Path('/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang');SOURCE=ROOT/'crates/emit_native/tests/fixtures/devirtualization/getter_return_provenance.dart';ARCHIVE=ROOT/'bench/results/2026-10-14-selector-getter-native'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert sha(CLI)==sys.argv[2]
OUT.mkdir(parents=True,exist_ok=True)
env=dict(os.environ,TMPDIR=str(OUT),DARTFORGE_CLANG=str(CLANG),LLVM_SYS_221_PREFIX=str(CLANG.parent.parent),DARTFORGE_SDK_LIB=str(SDK/'lib'),DARTFORGE_NO_PHYSICAL_GETTER_DEVIRTUALIZATION='1',DARTFORGE_KEEP_IR='1',DARTFORGE_LINK_MAP='1')
removed=['DARTFORGE_NO_SELECTOR_RETURN_PROVENANCE','DARTFORGE_ARC_PREPARED','DARTFORGE_ARC_PREPARE','DARTFORGE_ARC_CHECK','DARTFORGE_EFFECTS','DARTFORGE_GC_STRESS','DARTFORGE_NO_CANONICAL_CACHE_PROVENANCE','DARTFORGE_NO_PRE_PRUNE_CFG','DARTFORGE_NO_PLATFORM_SPECIALIZATION','DARTFORGE_NO_ENUM_PATTERN_IDENTITY']
for key in removed:env.pop(key,None)
expected='A:owner\nA:phi\nB:phi\nB:custom\nA:missing\ngetters:2\n[65]\n[195, 169]\nQUI=\n'
report={'compiler':str(CLI),'compiler_sha256':sha(CLI),'source':str(SOURCE),'source_sha256':sha(SOURCE),'SDK_binary_sha256':sha(SDK/'bin/dart'),'expected_stdout':expected,'environment':{k:v for k,v in env.items() if k.startswith('DARTFORGE_') or k in ['TMPDIR','LLVM_SYS_221_PREFIX']},'removed_environment':removed,'oracles':{},'profiles':{},'scope':'Four native executions (OFF/ON × normal/GCstress). No timing, GUI, broad parity or cached SDK getter propagation claim.'}
def save():
    text=json.dumps(report,indent=2)+'\n';(OUT/'validation.json').write_text(text);(ARCHIVE/'validation.json').write_text(text)
for name,command in [('vm',[str(SDK/'bin/dart'),'run',str(SOURCE)]),('official_aot_compile',[str(SDK/'bin/dart'),'compile','exe',str(SOURCE),'-o',str(OUT/'official')])]:
    executed=subprocess.run(command,capture_output=True,text=True,env=env,timeout=120)
    (OUT/(name+'.stdout')).write_text(executed.stdout);(OUT/(name+'.stderr')).write_text(executed.stderr)
    report['oracles'][name]={'command':command,'exit':executed.returncode,'stdout':executed.stdout,'stderr':executed.stderr};save();assert executed.returncode==0,report['oracles'][name]
    if name=='vm':assert executed.stdout==expected and not executed.stderr
executed=subprocess.run([str(OUT/'official')],capture_output=True,text=True,env=env,timeout=30)
report['oracles']['official_aot_run']={'exit':executed.returncode,'stdout':executed.stdout,'stderr':executed.stderr,'binary_sha256':sha(OUT/'official')};save();assert executed.returncode==0 and executed.stdout==expected and not executed.stderr
for profile in ['off','on']:
    folder=OUT/profile;folder.mkdir(exist_ok=True);binary=folder/'program';e=dict(env,DARTFORGE_WHY=str(folder/'retention.tsv'))
    if profile=='off':e['DARTFORGE_NO_SELECTOR_RETURN_PROVENANCE']='1'
    command=[str(CLI),'compile-native',str(SOURCE),'--sdk',str(SDK/'lib'),'--optimize','--memory=tracing','--exceptions=check','--roots=shadow','--stack-trace=none','-o',str(binary)]
    compiled=subprocess.run(command,capture_output=True,text=True,env=e,timeout=600)
    (folder/'compile.stdout').write_text(compiled.stdout);(folder/'compile.stderr').write_text(compiled.stderr)
    item={'command':command,'compile_exit':compiled.returncode,'opt_out':e.get('DARTFORGE_NO_SELECTOR_RETURN_PROVENANCE'),'effective_environment':{k:v for k,v in e.items() if k.startswith('DARTFORGE_') or k in ['TMPDIR','LLVM_SYS_221_PREFIX']}}
    report['profiles'][profile]=item;save();assert compiled.returncode==0,item
    item.update(binary_sha256=sha(binary),binary_bytes=binary.stat().st_size)
    for stress in [False,True]:
        runtime=e.copy()
        if stress:runtime['DARTFORGE_GC_STRESS']='1'
        result=subprocess.run([str(binary)],capture_output=True,text=True,env=runtime,timeout=60)
        name='stress' if stress else 'normal';(folder/(name+'.stdout')).write_text(result.stdout);(folder/(name+'.stderr')).write_text(result.stderr)
        item[name]={'exit':result.returncode,'stdout':result.stdout,'stderr':result.stderr,'exact':result.returncode==0 and result.stdout==expected and not result.stderr};save();assert item[name]['exact'],item[name]
    # Hash all retained IR/WHY artifacts; inspect local getter and convert callsites separately.
    item['artifacts']={str(path.relative_to(OUT)):{'sha256':sha(path),'bytes':path.stat().st_size} for path in folder.rglob('*') if path.is_file() and path.suffix in ['.ll','.poda','.tsv','.map']};save();print(profile,'correct',flush=True)
assert sha(CLI)==report['compiler_sha256'] and sha(SOURCE)==report['source_sha256'];report['inputs_stable']=True;save()
for folder in [OUT,OUT/'off',OUT/'on']:
    destination=ARCHIVE/folder.relative_to(OUT);destination.mkdir(parents=True,exist_ok=True)
    for path in folder.iterdir():
        if path.is_file() and path.suffix in ['.stdout','.stderr','.json','.tsv']:(destination/path.name).write_bytes(path.read_bytes())

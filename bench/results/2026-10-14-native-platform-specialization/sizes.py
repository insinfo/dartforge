"""Compile-only matched current File Explorer sizes; never execute GUI binaries."""
from pathlib import Path
from urllib.parse import urljoin,urlparse,unquote
import hashlib,json,os,struct,subprocess
ROOT=Path.cwd();OUT=ROOT/'target/platform-specialization/sizes';APP=Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui');SDK=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk');CLI=ROOT/'target/candidate-platform-producer/dartforge';CLANG=Path('/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang');ENTRY=APP/'examples/file_explorer/bin/file_explorer.dart';CONFIG=APP/'examples/file_explorer/.dart_tool/package_config.json'
OUT.mkdir(parents=True,exist_ok=True)
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert sha(CLI)=='539086b6334ac61ddbaea956ddf9218aca83d0435eb54633119f9318a88987d7'
# Include the previously inventoried sources plus all currently configured package libraries.
old=json.loads((ROOT/'bench/results/2026-10-14-native-constant-strings/file-explorer/inputs.json').read_text());files={Path(p) for p in old};files.update([ENTRY,CONFIG,SDK/'lib/libraries.json'])
files.update(APP.rglob('*.dart'));files.update((SDK/'lib').rglob('*.dart'))
for package in json.loads(CONFIG.read_text())['packages']:
    base=urljoin(CONFIG.as_uri(),package['rootUri']);lib=urljoin(base.rstrip('/')+'/',package.get('packageUri',''))
    directory=Path(unquote(urlparse(lib).path))
    if directory.is_dir():files.update(directory.rglob('*.dart'))
inputs={str(p):sha(p) for p in sorted(files) if p.is_file()};(OUT/'inputs.json').write_text(json.dumps(inputs,indent=2)+'\n')
env=dict(os.environ,TMPDIR=str(OUT),DARTFORGE_CLANG=str(CLANG),LLVM_SYS_221_PREFIX=str(CLANG.parent.parent),DARTFORGE_SDK_LIB=str(SDK/'lib'),DARTFORGE_LINK_MAP='1',DARTFORGE_FACTORY_RECEIVER_PROVENANCE='1',DARTFORGE_ARC_PORT_PRUNING='1')
removed=['DARTFORGE_NO_PLATFORM_SPECIALIZATION','DARTFORGE_ARC_CHECK','DARTFORGE_EFFECTS','DARTFORGE_GC_STRESS','DARTFORGE_ARC_PREPARED','DARTFORGE_ARC_PREPARE','DARTFORGE_MEMORY','DARTFORGE_NO_PROGRAM_PRUNING','DARTFORGE_NO_HIR_PRUNING','DARTFORGE_NO_TABLE_PRUNING','DARTFORGE_NO_FFI_PRUNING']
for k in removed:env.pop(k,None)
report={'entry':str(ENTRY),'package_config':str(CONFIG),'compiler':str(CLI),'compiler_sha256':sha(CLI),'SDK_version':subprocess.check_output([str(SDK/'bin/dart'),'--version'],stderr=subprocess.STDOUT,text=True).strip(),'SDK_binary_sha256':sha(SDK/'bin/dart'),'clang_sha256':sha(CLANG),'input_count':len(inputs),'input_manifest_sha256':sha(OUT/'inputs.json'),'environment':{k:env[k] for k in ['TMPDIR','DARTFORGE_CLANG','LLVM_SYS_221_PREFIX','DARTFORGE_SDK_LIB','DARTFORGE_LINK_MAP','DARTFORGE_FACTORY_RECEIVER_PROVENANCE','DARTFORGE_ARC_PORT_PRUNING']},'unset_environment':removed,'profiles':{},'scope':'Matched off/on platform specialization under one frozen compiler/runtime. Current compile-only size evidence. No execution, startup, GUI, throughput or compile-time performance claim. Full input inventory hashes before and after.'}
def sections(path):
    data=path.read_bytes();magic=struct.unpack_from('<I',data)[0]
    if magic!=0xfeedfacf:return {'unsupported_magic':hex(magic)}
    ncmds=struct.unpack_from('<I',data,16)[0];offset=32;result={}
    for _ in range(ncmds):
        cmd,size=struct.unpack_from('<II',data,offset)
        if cmd==0x19:
            count=struct.unpack_from('<I',data,offset+64)[0]
            for i in range(count):
                pos=offset+72+80*i;sec,seg,addr,length,fileoff=struct.unpack_from('<16s16sQQI',data,pos)
                label=seg.rstrip(b'\0').decode()+','+sec.rstrip(b'\0').decode();result[label]={'address':addr,'bytes':length,'offset':fileoff}
        offset+=size
    return result
for profile in ['off','on']:
    folder=OUT/profile;folder.mkdir(exist_ok=True);binary=folder/'program';e=dict(env)
    command=[str(CLI),'compile-native',str(ENTRY),'--sdk',str(SDK/'lib'),'--packages',str(CONFIG),'--optimize','--memory=tracing','--exceptions=check','--roots=shadow','--stack-trace=none','-o',str(binary)]
    if profile=='off':e['DARTFORGE_NO_PLATFORM_SPECIALIZATION']='1'
    run=subprocess.run(command,capture_output=True,text=True,env=e,cwd=ROOT,timeout=1200)
    (folder/'compile.stdout').write_text(run.stdout);(folder/'compile.stderr').write_text(run.stderr)
    item={'command':command,'exit':run.returncode,'NO_PLATFORM_SPECIALIZATION':e.get('DARTFORGE_NO_PLATFORM_SPECIALIZATION')}
    if run.returncode==0:
        item.update(binary=str(binary),binary_bytes=binary.stat().st_size,binary_sha256=sha(binary),sections=sections(binary))
        linkmap=binary.with_suffix('.map')
        if linkmap.exists():item.update(link_map=str(linkmap),link_map_bytes=linkmap.stat().st_size,link_map_sha256=sha(linkmap))
    report['profiles'][profile]=item;(OUT/'comparison.json').write_text(json.dumps(report,indent=2)+'\n');print(profile,run.returncode,item.get('binary_bytes'),flush=True)
    assert run.returncode==0,(profile,run.stderr[-3000:])
assert all(sha(p)==h for p,h in inputs.items()) and sha(CLI)==report['compiler_sha256']
report['inputs_unchanged']=True;report['compiler_unchanged']=True;(OUT/'comparison.json').write_text(json.dumps(report,indent=2)+'\n');print('compile-only terminal all inputs stable',flush=True)

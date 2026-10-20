import os,subprocess,json,hashlib,re
from pathlib import Path
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-string-length-reuse')
env=os.environ.copy();prefix='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64';env.update(DARTFORGE_LLVM_DIR=prefix,LLVM_SYS_221_PREFIX=prefix,SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk',MACOSX_DEPLOYMENT_TARGET='15.0',DARTFORGE_SDK_LIB='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib',DARTFORGE_NATIVE_CACHE=str(root/'target/string-length-native-cache'))
env['PATH']=prefix+'/bin:'+env['PATH'];env.pop('DARTFORGE_NO_STRING_LENGTH_REUSE',None)
compiler=root/'target/string-length-validation/debug/dartforge';runner=root/'target/debug/dartforge-run-ir';records=[]
for label,extra in [('dev-build',['-o',str(out/'development')]),('dev-ir',['--emit-ir','-o',str(out/'development.ll')])]:
 command=[str(compiler),'compile-native',str(out/'lengths.dart'),'--sdk',env['DARTFORGE_SDK_LIB'],*extra]
 result=subprocess.run(command,env=env,capture_output=True)
 (out/(label+'.stdout')).write_bytes(result.stdout);(out/(label+'.stderr')).write_bytes(result.stderr)
 records.append(dict(label=label,command=command,exit=result.returncode));(out/'jit-execution.json').write_text(json.dumps(records,indent=2)+'\n');print(label,result.returncode,flush=True)
 if result.returncode: raise SystemExit(1)
loads=subprocess.check_output(['otool','-L',str(out/'development')],text=True)
(out/'development-loads.txt').write_text(loads)
sdkline=next(line.strip().split(' (')[0] for line in loads.splitlines() if 'libdfsdk_' in line)
dll=Path(sdkline)
if sdkline.startswith('@rpath/'):
 load_commands=subprocess.check_output(['otool','-l',str(out/'development')],text=True)
 (out/'development-load-commands.txt').write_text(load_commands)
 candidates=[]
 for match in re.finditer(r'cmd LC_RPATH\n\s*cmdsize \d+\n\s*path (.*?) \(offset \d+\)',load_commands):
  base=match.group(1).replace('@executable_path',str(out)).replace('@loader_path',str(out))
  candidate=Path(base)/sdkline.removeprefix('@rpath/')
  if candidate.is_file(): candidates.append(candidate.resolve())
 candidates=list(set(candidates))
 if len(candidates)!=1: raise RuntimeError('Ambiguous SDK dependency: '+str(candidates))
 dll=candidates[0]
elif not dll.is_absolute(): raise RuntimeError('Unsupported SDK dependency: '+sdkline)
assert dll.is_file(),dll
env['DARTFORGE_SDK_DLL']=str(dll)
expected=(out/'dart.stdout').read_bytes()
for label,program in [('development-aot',[str(out/'development')]),('development-jit',[str(runner),str(out/'development.ll')])]:
 for stress in [False,True]:
  selected=env.copy();selected.pop('DARTFORGE_GC_STRESS',None)
  if stress:selected['DARTFORGE_GC_STRESS']='1'
  name=label+('-stress' if stress else '-normal');result=subprocess.run(program,env=selected,capture_output=True)
  (out/(name+'.stdout')).write_bytes(result.stdout);(out/(name+'.stderr')).write_bytes(result.stderr)
  matched=result.returncode==0 and result.stdout==expected
  records.append(dict(label=name,command=program,exit=result.returncode,matches_vm=matched,gc_stress=selected.get('DARTFORGE_GC_STRESS'),sdk_dll=str(dll)))
  (out/'jit-execution.json').write_text(json.dumps(records,indent=2)+'\n');print(name,result.returncode,matched,flush=True)
  if not matched: raise SystemExit(1)
(out/'jit-bindings.json').write_text(json.dumps({'runner_sha256':hashlib.sha256(runner.read_bytes()).hexdigest(),'sdk_dll':str(dll),'sdk_sha256':hashlib.sha256(dll.read_bytes()).hexdigest(),'ir_sha256':hashlib.sha256((out/'development.ll').read_bytes()).hexdigest()},indent=2)+'\n')

import os, subprocess, json, hashlib, re
from pathlib import Path
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge'); out=Path('/tmp/forge-string-length-reuse')
compiler=root/'target/string-length-validation/debug/dartforge'; source=out/'lengths.dart'; sdk='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib'
env=os.environ.copy();prefix='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64';env.update(DARTFORGE_LLVM_DIR=prefix,LLVM_SYS_221_PREFIX=prefix,SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk',MACOSX_DEPLOYMENT_TARGET='15.0',DARTFORGE_SDK_LIB=sdk,DARTFORGE_NATIVE_CACHE=str(root/'target/string-length-native-cache'))
env['PATH']=prefix+'/bin:/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin:'+env['PATH']
expected=(out/'dart.stdout').read_bytes(); records=[]
for label,disabled,arc in [('enabled',False,False),('disabled',True,False),('arc',False,True)]:
 selected=env.copy()
 if disabled: selected['DARTFORGE_NO_STRING_LENGTH_REUSE']='1'
 else: selected.pop('DARTFORGE_NO_STRING_LENGTH_REUSE',None)
 command=[str(compiler),'compile-native',str(source),'--sdk',sdk,'--optimize']
 if arc: command += ['--memory','arc']
 executable=out/label
 for stage,args in [('build',['-o',str(executable)]),('ir',['--emit-ir','-o',str(out/(label+'.ll'))])]:
  full=command+args;result=subprocess.run(full,env=selected,capture_output=True)
  (out/(label+'-'+stage+'.stdout')).write_bytes(result.stdout);(out/(label+'-'+stage+'.stderr')).write_bytes(result.stderr)
  records.append(dict(label=label,stage=stage,command=full,exit=result.returncode));(out/'execution.json').write_text(json.dumps(records,indent=2)+'\n')
  print(label,stage,result.returncode,flush=True)
  if result.returncode: raise SystemExit(1)
 for mode,args in [('normal',[]),('stress',['--gc-stress'])]:
  result=subprocess.run([str(executable),*args],env=selected,capture_output=True)
  (out/(label+'-'+mode+'.stdout')).write_bytes(result.stdout);(out/(label+'-'+mode+'.stderr')).write_bytes(result.stderr)
  matched=result.returncode==0 and result.stdout==expected
  records.append(dict(label=label,stage=mode,exit=result.returncode,matches_vm=matched));(out/'execution.json').write_text(json.dumps(records,indent=2)+'\n')
  print(label,mode,result.returncode,matched,flush=True)
  if not matched: raise SystemExit(1)
comparison={}
for label in ['enabled','disabled','arc']:
 text=(out/(label+'.ll')).read_text(); bodies={}
 for match in re.finditer(r'^define[^\n]*@([^\n]*?)\([^\n]*\)[^\n]*\{\n(.*?)^}',text,re.M|re.S):
  name,body=match.groups()
  if any(part in name for part in ['stable','changing','nullable']):
   bodies[name]={'length_calls':len(re.findall(r'call i64 @df\.string_len\(',body)),'sha256':hashlib.sha256(body.encode()).hexdigest()}
 comparison[label]=bodies
(out/'ir-comparison.json').write_text(json.dumps(comparison,indent=2)+'\n')
(out/'bindings.json').write_text(json.dumps({'compiler_sha256':hashlib.sha256(compiler.read_bytes()).hexdigest(),'fixture_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'expected_stdout':expected.decode(),'native_cache':env['DARTFORGE_NATIVE_CACHE']},indent=2)+'\n')

from pathlib import Path
import subprocess,json,os
s=Path('/tmp/forge-stack-storage/check.py').read_text();exec(s[:s.index('commands=')]);out=Path('/tmp/forge-pruned-jit');scratch=root/'target/tmp-pruned-jit';scratch.mkdir(exist_ok=True)
env.update(DARTFORGE_NATIVE_CACHE=str(root/'target/rpo-native-cache'),DARTFORGE_TEST_SDK_LIB=env['DARTFORGE_SDK_LIB'],DARTFORGE_COMPILER=str(root/'target/test-tools/dartforge-stack-storage'),DARTFORGE_NATIVE_COMPILER=str(root/'target/test-tools/dartforge-stack-storage'),DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang',TMPDIR=str(scratch),RUST_TEST_THREADS='2',RUST_BACKTRACE='1');env.pop('DARTFORGE_SDK_DLL',None)
command=['cargo','test','--locked','-p','dartforge-jit','--','--include-ignored']
(out/'test-environment.json').write_text(json.dumps({k:env[k] for k in ['DARTFORGE_LLVM_DIR','LLVM_SYS_221_PREFIX','SDKROOT','MACOSX_DEPLOYMENT_TARGET','DARTFORGE_NATIVE_CACHE','DARTFORGE_TEST_SDK_LIB','DARTFORGE_COMPILER','DARTFORGE_NATIVE_COMPILER','DARTFORGE_CLANG','TMPDIR','RUST_TEST_THREADS','RUSTDOCFLAGS']},indent=2)+'\n')
with (out/'jit-tests.log').open('w') as log:p=subprocess.run(command,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
(out/'jit-tests.json').write_text(json.dumps(dict(command=command,exit=p.returncode),indent=2)+'\n');print('JIT tests',p.returncode,flush=True)

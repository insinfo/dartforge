from pathlib import Path
import subprocess,os,json,hashlib
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge')
out=root/'bench/results/2026-10-19-example-target-names'
env=os.environ.copy()
env.update({'DARTFORGE_LLVM_DIR':'/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64','LLVM_SYS_221_PREFIX':'/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64','SDKROOT':'/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk','MACOSX_DEPLOYMENT_TARGET':'15.0'})
env['PATH']='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin:'+env['PATH']
rows=[]
def run(label,command,expected=0):
 result=subprocess.run(command,cwd=root,env=env,capture_output=True,text=True)
 (out/(label+'.stdout')).write_text(result.stdout)
 (out/(label+'.stderr')).write_text(result.stderr)
 rows.append({'label':label,'command':command,'exit':result.returncode,'expected_exit':expected})
 (out/'runs.json').write_text(json.dumps(rows,indent=2)+'\n')
 assert result.returncode==expected,(label,result.stderr)
 return result
run('build',['cargo','build','--locked','-p','dartforge-ng-generator','-p','dartforge-formatter','--examples'])
ng=run('ng-usage',[str(root/'target/debug/examples/ng_oracle')],1)
assert ng.stderr=='usage: ng_oracle <project root> [--list] [--all] [--dump <dir>]\n'
fixture=Path('/tmp/dartforge-example-target-names');fixture.mkdir(exist_ok=True)
(fixture/'main.dart').write_text("void main() {\n  print('oracle');\n}\n")
run('formatter',[str(root/'target/debug/examples/oracle'),'--dart','/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart','--cache',str(fixture/'cache'),str(fixture/'main.dart')])
(out/'binaries.json').write_text(json.dumps([{'path':str(root/'target/debug/examples'/name),'sha256':hashlib.sha256((root/'target/debug/examples'/name).read_bytes()).hexdigest()} for name in ['ng_oracle','oracle']],indent=2)+'\n')
print('Both examples built and executed under distinct target names.')

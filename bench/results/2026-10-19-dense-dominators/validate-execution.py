from pathlib import Path
import os,subprocess,json,hashlib
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=root/'target/dense-dominator-validation';prefix='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64'
env=os.environ.copy();env.update({'SDKROOT':'/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk','MACOSX_DEPLOYMENT_TARGET':'15.0'});env['PATH']=prefix+'/bin:/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin:'+env['PATH']
rows=[]
def run(label,command,extra=None):
 current=env.copy();current.update(extra or {});result=subprocess.run(command,cwd=root,env=current,capture_output=True)
 (out/(label+'.stdout')).write_bytes(result.stdout);(out/(label+'.stderr')).write_bytes(result.stderr)
 rows.append({'label':label,'command':command,'environment_override':extra or {},'exit':result.returncode,'stdout':result.stdout.decode(),'stderr':result.stderr.decode()});(out/'execution.json').write_text(json.dumps(rows,indent=2)+'\n')
 assert result.returncode==0,(label,result.stderr.decode());return result
oracle=run('vm',['/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart','run',str(out/'branches.dart')])
for label,name in [('old','dartforge'),('dense','dartforge-dense-dominators')]:
 compiler=root/'target/test-tools'/name;exe=out/(label+'.exe')
 run(label+'-compile',[str(compiler),'compile-native',str(out/'branches.dart'),'--sdk','/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib','-o',str(exe)])
 for suffix,extra in [('normal',{}),('stress',{'DARTFORGE_GC_STRESS':'1'})]:
  result=run(label+'-'+suffix,[str(exe)],extra);assert result.stdout==oracle.stdout and not result.stderr
print('Both compilers produce executables matching Dart VM normally and under GC stress.')

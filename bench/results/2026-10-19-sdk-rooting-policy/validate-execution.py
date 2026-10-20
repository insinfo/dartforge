from pathlib import Path
import os,subprocess,json,hashlib
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=root/'target/root-policy-validation';source=root/'target/dense-dominator-validation/branches.dart';compiler=root/'target/test-tools/dartforge-root-policy-fixed';prefix='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64'
env=os.environ.copy();env.update({'SDKROOT':'/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk','MACOSX_DEPLOYMENT_TARGET':'15.0','DARTFORGE_NATIVE_CACHE':str(out/'native-cache'),'DARTFORGE_MEMORY':'tracing','DARTFORGE_ROOTS':'shadow'});env['PATH']=prefix+'/bin:/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin:'+env['PATH'];env.pop('DARTFORGE_OPTIMIZE_HIR',None);env.pop('DARTFORGE_ROOTS_IN_CALLEE',None)
rows=[]
def run(label,command,extra=None):
 current=env.copy();current.update(extra or {});result=subprocess.run(command,cwd=root,env=current,capture_output=True)
 (out/(label+'.stdout')).write_bytes(result.stdout);(out/(label+'.stderr')).write_bytes(result.stderr)
 rows.append({'label':label,'command':command,'environment_override':extra or {},'exit':result.returncode,'stdout':result.stdout.decode(),'stderr':result.stderr.decode()});(out/'execution.json').write_text(json.dumps(rows,indent=2)+'\n')
 assert result.returncode==0,(label,result.stderr.decode());return result
oracle=run('vm',['/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart','run',str(source)])
for label,mode in [('caller','0'),('callee','1')]:
 exe=out/(label+'.exe');run(label+'-compile',[str(compiler),'compile-native',str(source),'--sdk','/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib','-o',str(exe)],{'DARTFORGE_ROOTS_IN_CALLEE':mode})
 for suffix,extra in [('normal',{}),('stress',{'DARTFORGE_GC_STRESS':'1'})]:
  result=run(label+'-'+suffix,[str(exe)],extra);assert result.stdout==oracle.stdout and not result.stderr
print('Both rooting modes match Dart VM normally and under GC stress.')

from pathlib import Path
import os,subprocess,json,hashlib,time,shlex,base64,sys
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-sdk-image-owner-runtime');artifacts=root/'target/sdk-image-owner-validation';compiler=root/'target/test-tools/dartforge-sdk-image-owner';sdk='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib';source=out/'static-startup.dart';prefix='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64';env=os.environ.copy();env.update(DARTFORGE_LLVM_DIR=prefix,LLVM_SYS_221_PREFIX=prefix,DARTFORGE_GENERATOR='clang',DARTFORGE_NATIVE_CACHE=str(root/'target/rpo-native-cache'),SDKROOT='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk',MACOSX_DEPLOYMENT_TARGET='15.0');env['PATH']=prefix+'/bin:/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin:'+env['PATH'];reference=json.loads((out/'after-runtime.json').read_text())[0]['stdout'];records=json.loads((out/'cross-runtime.json').read_text()) if (out/'cross-runtime.json').exists() else []
def call(name,command):
 start=time.monotonic();result=subprocess.run(command,env=env,capture_output=True,text=True);r=dict(name=name,command=command,exit=result.returncode,seconds=time.monotonic()-start,stdout=result.stdout,stderr=result.stderr);records.append(r);(out/'cross-runtime.json').write_text(json.dumps(records,indent=2)+'\n');print(name,result.returncode,flush=True);assert result.returncode==0,r;return result.stdout
for target in (sys.argv[1:] or ['linux-arm64','windows-x64']):
 exe=artifacts/('cross-'+target+('.exe' if target=='windows-x64' else ''))
 call(target+'-compile',[str(compiler),'compile-native',str(source),'--optimize','--target',target,'--target-bundle',str(root/'target/cross-bundles/sdk-image-owner'/target),'--sdk',sdk,'--timings','-o',str(exe)])
 sha=hashlib.sha256(exe.read_bytes()).hexdigest()
 if target=='linux-arm64':
  shared='/media/psf/ssd_dock/MyRustProjects/dartforge/'+str(exe.relative_to(root))
  program="""import os,pathlib,hashlib,shutil,subprocess,json
source=pathlib.Path(SOURCE);dest=pathlib.Path.home()/'.cache/dartforge/sdk-image-owner'/SHA
assert hashlib.sha256(source.read_bytes()).hexdigest()==SHA
dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(source,dest);dest.chmod(0o755)
rows=[]
for stress in [False,True]:
 env=os.environ.copy();env.pop('DARTFORGE_GC_STRESS',None)
 if stress:env['DARTFORGE_GC_STRESS']='1'
 result=subprocess.run([str(dest)],env=env,capture_output=True,text=True)
 rows.append(dict(stress=stress,exit=result.returncode,stdout=result.stdout,stderr=result.stderr,sha256=hashlib.sha256(dest.read_bytes()).hexdigest()))
print(json.dumps(rows,ensure_ascii=False))
""".replace('SOURCE',repr(shared)).replace('SHA',repr(sha))
  shell="runuser -u isaque -- /usr/bin/python3 - <<'PYCODE'\n"+program+'PYCODE\n';(out/'linux-runtime.sh').write_text(shell)
  text=call(target+'-run',['prlctl','exec','Ubuntu24.04_arm64','/bin/bash','-lc',shlex.quote(shell)])
 else:
  shared='\\\\Mac\\ssd_dock\\MyRustProjects\\dartforge\\'+str(exe.relative_to(root)).replace('/','\\')
  shell=r"""$ErrorActionPreference='Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$source='SOURCE';$sha='SHA'
$dest=Join-Path $env:LOCALAPPDATA ('DartForgeSDKImageOwner\'+$sha+'.exe')
New-Item -ItemType Directory -Force (Split-Path $dest) | Out-Null
if((Get-FileHash $source -Algorithm SHA256).Hash.ToLowerInvariant() -ne $sha){throw 'source hash mismatch'}
Copy-Item -LiteralPath $source -Destination $dest -Force
$rows=@()
foreach($stress in @($false,$true)) {
 $info=[Diagnostics.ProcessStartInfo]::new();$info.FileName=$dest;$info.UseShellExecute=$false
 $info.CreateNoWindow=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
 $info.StandardOutputEncoding=[Text.UTF8Encoding]::new($false);$info.StandardErrorEncoding=[Text.UTF8Encoding]::new($false)
 $info.EnvironmentVariables.Remove('DARTFORGE_GC_STRESS')
 if($stress){$info.EnvironmentVariables['DARTFORGE_GC_STRESS']='1'}
 $p=[Diagnostics.Process]::new();$p.StartInfo=$info;[void]$p.Start()
 $stdout=$p.StandardOutput.ReadToEnd();$stderr=$p.StandardError.ReadToEnd();$p.WaitForExit()
 $rows+=@{stress=$stress;exit=$p.ExitCode;stdout=$stdout;stderr=$stderr;sha256=(Get-FileHash $dest -Algorithm SHA256).Hash.ToLowerInvariant()}
}
ConvertTo-Json -Depth 8 -Compress -InputObject $rows
""".replace('SOURCE',shared).replace("'SHA'", "'"+sha+"'")
  (out/'windows-runtime.ps1').write_text(shell);(artifacts/'windows-runtime.ps1').write_text(shell)
  text=call(target+'-run',['prlctl','exec','Windows11','--current-user','powershell.exe','-NoLogo','-NoProfile','-NonInteractive','-OutputFormat','Text','-ExecutionPolicy','Bypass','-File',r'\\Mac\ssd_dock\MyRustProjects\dartforge\target\sdk-image-owner-validation\windows-runtime.ps1'])
 rows=json.loads(text);assert len(rows)==2,rows
 for row in rows:
  assert row['exit']==0 and row['sha256']==sha and row['stdout'].replace('\r\n','\n')==reference and not row['stderr'],row
 records[-1]['parsed']=rows;(out/'cross-runtime.json').write_text(json.dumps(records,indent=2)+'\n')

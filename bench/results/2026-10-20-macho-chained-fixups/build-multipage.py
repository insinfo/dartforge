from pathlib import Path
import subprocess,hashlib,json
out=Path('/tmp/forge-chained-fixups');prefix=Path('/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin')
source=(out/'fixups.c').read_text();source+='\nconst void *many[] = {'+','.join(['strings']*2100)+'};\n'
(out/'multipage.c').write_text(source)
records=[]
for arch,target in [('arm64','aarch64-apple-darwin'),('x86_64','x86_64-apple-darwin')]:
 obj=out/('multi-'+arch+'.o');image=out/('multi-'+arch)
 commands=[[str(prefix/'clang'),'-target',target,'-mmacosx-version-min=13.0','-c',str(out/'multipage.c'),'-o',str(obj)],[str(prefix/'ld64.lld'),'-arch',arch,'-platform_version','macos','13.0','13.0','-e','_entry','-undefined','dynamic_lookup','-fixup_chains',str(obj),'-o',str(image)]]
 for command in commands:
  result=subprocess.run(command,capture_output=True,text=True);assert result.returncode==0,(command,result.stderr)
  records.append(dict(command=command,exit=result.returncode,stdout=result.stdout,stderr=result.stderr))
 records.append(dict(image=str(image),bytes=image.stat().st_size,sha256=hashlib.sha256(image.read_bytes()).hexdigest(),executed=False))
(out/'multipage-builds.json').write_text(json.dumps(records,indent=2)+'\n')
print('Built ARM64/x64 chained images with 2,100 extra pointers; images not executed.')

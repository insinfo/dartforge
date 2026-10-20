from pathlib import Path
import subprocess,json,hashlib,os
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');artifacts=root/'target/pe-relocation-validation';artifacts.mkdir(exist_ok=True);out=Path('/tmp/forge-pe-checks');prefix=Path('/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin');inspector=root/'target/string-length-validation/debug/examples/inspect_linked_images'
# An empty permanent one-byte string: one body word, canonical empty hash 1.
header=(6<<32)|(1<<16)|(2<<8)|5
source=artifacts/'pointers.c';source.write_text('__attribute__((section(".dfimg$m"), aligned(8))) const unsigned long long strings[] = {'+str(header)+'ULL,1,0};\nconst void *pointers[2100] = {'+','.join(['strings']*2100)+'};\nconst void *entry(void) { return pointers[0]; }\n')
records=[]
for target in ['x86_64-pc-windows-msvc','aarch64-pc-windows-msvc']:
 obj=artifacts/(target+'.obj');exe=artifacts/(target+'.exe')
 commands=[[str(prefix/'clang'),'-target',target,'-c',str(source),'-o',str(obj)],[str(prefix/'lld-link'),'/entry:entry','/subsystem:console','/nodefaultlib','/dynamicbase',str(obj),'/out:'+str(exe)],[str(inspector),str(exe)]]
 for command in commands:
  p=subprocess.run(command,capture_output=True,text=True);records.append(dict(command=command,exit=p.returncode,stdout=p.stdout,stderr=p.stderr));(out/'linked-fixtures.json').write_text(json.dumps(records,indent=2)+'\n');assert p.returncode==0,p.stderr
 audit=json.loads(p.stdout)['pe_relocations'];assert audit['potential_writes']==2100 and audit['overlapping_writes']==0 and audit['blocks']==5
records.append(dict(source=str(source),source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),executed=False))
(out/'linked-fixtures.json').write_text(json.dumps(records,indent=2)+'\n')

#!/usr/bin/python3
import os,sys,json,hashlib,shutil
from pathlib import Path
out=Path('/Volumes/ssd_dock/MyRustProjects/dartforge/target/relr-explorer')
copies=out/'inputs';copies.mkdir(exist_ok=True)
args=sys.argv[1:];records=[]
for index,value in enumerate(args):
    path=Path(value)
    if path.is_file() and (path.suffix in ('.o','.obj','.a') or '.so' in path.name):
        target=copies/(str(index)+'-'+path.name)
        shutil.copy2(path,target)
        digest=hashlib.sha256(target.read_bytes()).hexdigest()
        records.append({'original':str(path),'copy':str(target),'sha256':digest,'bytes':target.stat().st_size})
        args[index]=str(target)
(out/'input-bindings.json').write_text(json.dumps(records,indent=2)+'\n')
(out/'baseline-link.json').write_text(json.dumps(['/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/ld.lld']+args,indent=2)+'\n')
os.execv('/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/ld.lld',['/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/ld.lld']+args)

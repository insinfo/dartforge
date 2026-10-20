from pathlib import Path
import subprocess,json,hashlib,re
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-sdk-image-owner-runtime');records=[]
for label in ['before','after']:
 exe=root/('target/sdk-image-owner-validation/'+label+'-development');command=['otool','-L',str(exe)];p=subprocess.run(command,capture_output=True,text=True);assert p.returncode==0,p.stderr
 prefix=re.search('libdfsdk_([0-9a-f]{16})\\.dylib',p.stdout).group(1);candidates=list((root/'target/rpo-native-cache/sdk').glob(prefix+'*'));assert len(candidates)==1
 directory=candidates[0];objects=[]
 for name in ['_internal','core','_compact_hash','collection','math','convert','async','typed_data','isolate','developer','nativewrappers','ffi','_http','io','mirrors']:
  obj=directory/(name+'.o');command=['otool','-rv',str(obj)];p=subprocess.run(command,capture_output=True,text=True);assert p.returncode==0,p.stderr
  matches=[line.strip() for line in p.stdout.splitlines() if ' _dartforge_register_image' in line]
  objects.append(dict(module=name,path=str(obj),sha256=hashlib.sha256(obj.read_bytes()).hexdigest(),command=command,exit=p.returncode,relocation_output_bytes=len(p.stdout.encode()),relocation_output_sha256=hashlib.sha256(p.stdout.encode()).hexdigest(),image_call_relocations=matches))
 counts=[len(v['image_call_relocations']) for v in objects];assert counts==([1]*15 if label=='before' else [1]+[0]*14),counts
 records.append(dict(label=label,sdk_directory=str(directory),image_call_relocations=sum(counts),objects=objects))
(out/'owner-objects.json').write_text(json.dumps(records,indent=2)+'\n');print([(r['label'],r['image_call_relocations']) for r in records])

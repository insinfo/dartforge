import pathlib,subprocess,re,json
app=pathlib.Path('target/native-provenance/pooled-indexed-app/program');stamp=app.stat().st_mtime
objects=[f for f in pathlib.Path('target/p4/native_cache/thinlto').glob('llvmcache-*') if stamp-25<f.stat().st_mtime<stamp+2]
rows=[];examples=[]
for obj in objects:
 rel=subprocess.check_output(['xcrun','otool','-rv',str(obj)],text=True)
 calls={int(m[1],16):m[2] for line in rel.splitlines() if (m:=re.match(r'([0-9a-f]+).*BR26.* (_dartforge_arc_(?:release|retain))$',line))}
 if not calls: continue
 dis=subprocess.check_output(['xcrun','otool','-tvV',str(obj)],text=True).splitlines()
 instructions=[(int(m[1],16),m[2]) for line in dis if (m:=re.match(r'([0-9a-f]{16})\s+(.+)$',line))]
 offsets={a:i for i,(a,_) in enumerate(instructions)};zero=[]
 for address,kind in calls.items():
  i=offsets.get(address)
  if i is None:continue
  for j in range(i-1,max(-1,i-5),-1):
   code=instructions[j][1]
   if re.match(r'(?:b\b|b\.|bl\b|blr\b|ret\b|cb|tb)',code):break
   if re.match(r'\S+\s+[xw]0[,\s]',code):
    if re.match(r'mov\s+[xw]0, (?:[xw]zr|#(?:0x0|0))$',code):
     zero.append(kind)
     if len(examples)<8:examples.append({'object':str(obj),'address':hex(address),'kind':kind,'instructions':[s for _,s in instructions[j:i+1]]})
    break
 rows.append({'object':str(obj),'release_sites':sum(k.endswith('release') for k in calls.values()),'retain_sites':sum(k.endswith('retain') for k in calls.values()),'physical_null_release_sites':zero.count('_dartforge_arc_release'),'physical_null_retain_sites':zero.count('_dartforge_arc_retain')})
result={'selection':'ThinLTO objects created within 25 seconds before completed immutable pooled FileExplorer output','compiler_binary':'19eb472 (parent immutable pooled/indexed checkpoint)','rows':rows,'examples':examples}
pathlib.Path('target/native-provenance/arc-null-machine-sites.json').write_text(json.dumps(result,indent=2)+'\n')
print({key:sum(row[key] for row in rows) for key in ['release_sites','retain_sites','physical_null_release_sites','physical_null_retain_sites']})
print(json.dumps(examples[:3],indent=2))

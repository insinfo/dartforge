import struct,pathlib,json,hashlib
root=pathlib.Path.cwd();sha=lambda b:hashlib.sha256(b).hexdigest();out=root/'target/rea-native-analysis'
for label,p in [('official-bench',root/'target/current-native-status/official/bench'),('official-explorer',root/'target/current-native-size/official/program')]:
 b=p.read_bytes();offset=0x43c000;x=b[offset:];assert x[:6]==b'\x7fELF\x02\x01';h=struct.unpack_from('<16sHHIQQQIHHHHHH',x);phoff,shoff=h[5:7];phsize,phnum,shsize,shnum,shstr=h[9:14];sections=[struct.unpack_from('<IIQQQQIIQQ',x,shoff+i*shsize) for i in range(shnum)];maxend=max([shoff+shsize*shnum]+[s[4]+s[5] for s in sections if s[1]!=8]+[struct.unpack_from('<IIQQQQQQ',x,phoff+i*phsize)[2]+struct.unpack_from('<IIQQQQQQ',x,phoff+i*phsize)[5] for i in range(phnum)]);assert maxend<=len(x);x=x[:maxend]
 names=x[sections[shstr][4]:sections[shstr][4]+sections[shstr][5]];name=lambda i,n:n[i:n.index(b'\0',i)].decode(errors='replace');rows=[];symbols=[]
 for s in sections:
  rows.append({'name':name(s[0],names),'type':s[1],'address':hex(s[3]),'offset':s[4],'bytes':s[5]})
  if s[1] in [2,11]:
   st=sections[s[6]];strings=x[st[4]:st[4]+st[5]]
   for i in range(s[5]//s[9]):
    q=struct.unpack_from('<IBBHQQ',x,s[4]+i*s[9]);symbols.append({'name':name(q[0],strings),'info':q[1],'section':q[3],'address':hex(q[4]),'bytes':q[5]})
 child=out/(label+'-snapshot.elf');child.write_bytes(x);meta={'original':str(p),'original_sha256':sha(b),'original_bytes':len(b),'file_offset':offset,'child':str(child),'child_sha256':sha(x),'child_bytes':len(x),'ELF_machine':h[2],'sections':rows,'symbols':symbols,'scope':'Exact ELF section/segment-bounded extraction from immutable official executable; custom Dart AOT ABI is not ordinary C.'};(out/(label+'-snapshot.json')).write_text(json.dumps(meta,indent=2)+'\n');print(label,maxend,rows);print(symbols[:12])

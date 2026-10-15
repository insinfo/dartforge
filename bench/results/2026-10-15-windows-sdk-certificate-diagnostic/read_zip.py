import urllib.request,struct,json,zlib,hashlib,pathlib
u='https://storage.googleapis.com/dart-archive/channels/stable/release/3.6.2/sdk/dartsdk-windows-x64-release.zip'
out=pathlib.Path('target/release-ci-audit/windows-sdk-source-audit')
def read(a,b):
 r=urllib.request.urlopen(urllib.request.Request(u,headers={'Range':f'bytes={a}-{b}'}));d=r.read();assert len(d)==b-a+1,(a,b,len(d));return d
n=222667616; tail=read(n-65536,n-1); e=tail.rfind(b'PK\x05\x06');_,_,_,count,_,size,off,_=struct.unpack_from('<4s4H2IH',tail,e)
c=read(off,off+size-1);i=0;entries=[]
while i<len(c):
 h=struct.unpack_from('<4s6H3I5H2I',c,i);fn,extra,comment=h[10:13];name=c[i+46:i+46+fn].decode();entries.append((name,h[8],h[9],h[-1]));i+=46+fn+extra+comment
selected=[x for x in entries if x[0].startswith('dart-sdk/lib/core/') or x[0]=='dart-sdk/lib/libraries.json']
results=[]
for name,compressed,size,offset in selected:
 header=read(offset,offset+29);fn,extra=struct.unpack_from('<HH',header,26);a=offset+30+fn+extra;raw=zlib.decompress(read(a,a+compressed-1),-15)
 local=pathlib.Path('/Volumes/ssd_dock/tools/dart/3.6.2')/name
 lb=local.read_bytes();results.append({'name':name,'equal':raw==lb,'windows_crlf':raw.count(b'\r\n'),'local_crlf':lb.count(b'\r\n'),'windows_sha256':hashlib.sha256(raw).hexdigest(),'local_sha256':hashlib.sha256(lb).hexdigest()})
 (out/pathlib.Path(name).name).write_bytes(raw)
(out/'result.json').write_text(json.dumps(results,indent=2));print(json.dumps({'files':len(results),'different':[x for x in results if not x['equal']]}))

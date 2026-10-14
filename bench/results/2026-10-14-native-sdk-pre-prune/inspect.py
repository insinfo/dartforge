"""Inspect current linked Mach-O sections and map text symbols without executing binaries."""
from pathlib import Path
import hashlib,json,re,shutil
ROOT=Path.cwd();OUT=ROOT/'target/sdk-pre-prune-sizes';DEST=ROOT/'bench/results/2026-10-14-native-sdk-pre-prune/sizes';m=json.loads((OUT/'comparison.json').read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
for name,item in m['profiles'].items():
    if item['exit']!=0:continue
    text=item['sections'].get('__TEXT,__text');item['text_section_bytes']=text['bytes'] if text else None
    if 'link_map' in item:
        path=Path(item['link_map']);symbols=[];objects={};active=False
        for line in path.read_text().splitlines():
            match=re.match(r'^\[\s*(\d+)\]\s+(.*)',line)
            if match:objects[int(match[1])]=match[2]
            if line=='# Symbols:':active=True;continue
            if line.startswith('# Dead Stripped Symbols:'):active=False
            if active:
                match=re.match(r'^(0x[0-9A-Fa-f]+)\s+(0x[0-9A-Fa-f]+)\s+\[\s*(\d+)\]\s+(.*)',line)
                if match:
                    address,size,object_id=int(match[1],16),int(match[2],16),int(match[3]);
                    if text and text['address']<=address<text['address']+text['bytes']:symbols.append({'address':address,'bytes':size,'object':object_id,'name':match[4]})
        item['text_symbol_entries']=len(symbols);item['largest_text_symbols']=sorted(symbols,key=lambda x:x['bytes'],reverse=True)[:20]
        for symbol in item['largest_text_symbols']:symbol['object_path']=objects.get(symbol['object'])
        item['map_scope']='Individual symbol spans can overlap due aliases. Text section total is authoritative; spans are not summed.'
        linked={}
        for path in objects.values():
            match=re.match(r'^(.*\.a)(?:\(|$)',path)
            if match and Path(match[1]).is_file():linked[match[1]]=sha(Path(match[1]))
        item['linked_static_archives_sha256']=linked
m['section_comparison_limit']='Mach-O section names and totals are layout facts. Official Dart includes VM and AOT snapshot images; __TEXT,__text alone is not a comparable total Dart instruction footprint.'
m['inspection_scope']='Raw binary and linked section/symbol sizes only. No runtime performance or execution claim.'
(OUT/'comparison.json').write_text(json.dumps(m,indent=2)+'\n')
for p in OUT.rglob('*'):
    if p.is_file() and p.suffix in ['.json','.stdout','.stderr','.log'] and p.name != 'program.contracts.json':
        destination=DEST/p.relative_to(OUT);destination.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(p,destination)
print([(n,i.get('binary_bytes'),i.get('text_section_bytes'))for n,i in m['profiles'].items()]);print('Raw logs need force-add; no binaries/maps archived.')

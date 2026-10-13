import json,os,collections
sel="c01 c02 c03 c04 c05 c06 c07 c08 c09 c10 c11 c12 c13 c14 c15 c16 c17 c18 c19 c20 c21 c22 c23 c24 c25 c27 c40 c42 c43 c57 c59 c60 c63 c66 c72 c73 c76 c77 c78 c80 c92 c93 c94 c95 c102 c107 c111 c123 c125 c129 c183 d10 d11 d14 d54 d57 d37 e27 e28 e50".split()
by={}
src={}
for d in 'abc':
    raw=open(f'{d}/out.json',encoding='utf8').read()
    j=json.loads(raw[raw.find('{"version"'):])
    for f in os.listdir(d):
        if f.endswith('.dart'):
            k=f[:-5]; src[k]=open(f'{d}/{f}',encoding='utf8').read().rstrip('\n'); by.setdefault(k,[])
    for x in j['diagnostics']:
        l=x['location']; r=l['range']; k=os.path.basename(l['file'])[:-5]
        by[k].append((r['start']['offset'],r['end']['offset']-r['start']['offset'],r['start']['line'],r['start']['column'],x['code'],x['problemMessage']))
out=['| caso | entrada | diagnósticos (código · offset+length · linha:coluna · mensagem) |','|---|---|---|']
for k in sel:
    ds=sorted(by[k])
    cell='<br>'.join(f"`{c}` · {o}+{n} · {li}:{co} · {m}" for o,n,li,co,c,m in ds) or '(nenhum)'
    out.append(f"| `{k}` | `{src[k].replace(chr(10),' ⏎ ')}` | {cell} |")
open('exemplos.md','w',encoding='utf8',newline='\n').write('\n'.join(out)+'\n')
print(len(sel))

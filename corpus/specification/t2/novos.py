import json,re,collections,io,sys,os
sys.stdout=io.TextIOWrapper(sys.stdout.buffer,encoding='utf-8')
B=chr(92)
sn=json.load(open('E:/MyRustProjects/dartforge/corpus/diagnosticos/sintaxe-nova.json',encoding='utf-8'))
novos=set(sn['analyzer'])|set(sn['linguagem'])
for g in ['analyzer-3.13','linguagem-3.13']:
    base='E:/MyRustProjects/dartforge/corpus/diagnosticos/'+g
    for r,d,f in os.walk(base):
        for x in f:
            if x.endswith('.dart'): novos.add(os.path.relpath(os.path.join(r,x),base).replace(B,'/'))
cnt=collections.defaultdict(lambda: collections.Counter()); tot=collections.Counter()
ex=collections.defaultdict(list)
for l in open('E:/dftemp/analise/trab/placar-r7.txt',encoding='utf-8'):
    m=re.match(r"  \[(\w+)\] (FN|FP|mensagem|posição): (\S+?\.dart):(\d+):(\d+) (.*)",l)
    if not m: continue
    c,k,f=m.group(1),m.group(2),m.group(3)
    tot[c]+=1
    if f in novos:
        cnt[c][k]+=1
        if len(ex[c])<3: ex[c].append(l.strip()[:260])
print('amostras total',sum(tot.values()),'em arquivos 3.13.4',sum(sum(v.values()) for v in cnt.values()))
for c,v in sorted(cnt.items(), key=lambda kv:-sum(kv[1].values())):
    print(c, sum(v.values()), dict(v), 'de', tot[c])
    for e in ex[c][:2]: print('     ',e)

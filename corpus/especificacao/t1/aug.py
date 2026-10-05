import re,os,sys,io,json,collections
sys.stdout=io.TextIOWrapper(sys.stdout.buffer,encoding='utf-8')
base=r'E:\MyRustProjects\dartforge\corpus\diagnosticos'
nova=json.load(open(os.path.join(base,'sintaxe-nova.json'),encoding='utf-8'))
novaset=set()
for g,l in nova.items():
    for p in l: novaset.add(os.path.basename(p))
idx={}
for g in ('analyzer','linguagem','analyzer-3.13','linguagem-3.13'):
    for r,d,fs in os.walk(os.path.join(base,g)):
        for f in fs: idx.setdefault(f,os.path.join(r,f))
rx=re.compile(r'^  \[(\w+)\] (FN|FP|mensagem|posição): ([^:]+\.dart):(\d+):(\d+)')
decl=re.compile(r'^\s*(?:augment\s+)?(?:abstract\s+|base\s+|final\s+|sealed\s+|interface\s+|mixin\s+)*(class|mixin|enum|extension type)\s+(\w+)',re.M)
cache={}
def classe(f):
    if f in cache: return cache[f]
    p=idx.get(f)
    r=set()
    if p:
        s=open(p,encoding='utf-8',errors='replace').read()
        if re.search(r'^\s*augment\s',s,re.M): r.add('AUG')
        names=collections.Counter((m.group(2)) for m in decl.finditer(s))
        if any(v>1 for v in names.values()): r.add('HOM')
        if f in novaset: r.add('NOVA')
    cache[f]=r
    return r
tot=collections.defaultdict(lambda: collections.Counter())
amostras=collections.defaultdict(list)
for l in open(r'E:\dftemp\analise\trab\placar-r7.txt',encoding='utf-8'):
    m=rx.match(l)
    if not m: continue
    c,k,f,li,co=m.groups(); f=f.split("/")[-1]
    r=classe(f)
    if ('AUG' in r or 'HOM' in r) and 'NOVA' not in r:
        tot[c][k]+=1
        amostras[c].append('%s %s:%s:%s'%(k,f,li,co))
for c in sorted(tot,key=lambda c:-sum(tot[c].values())):
    print(c,dict(tot[c]))
    for a in amostras[c][:6]: print('    ',a)
print('total',sum(sum(v.values()) for v in tot.values()))

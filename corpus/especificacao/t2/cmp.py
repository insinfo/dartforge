import yaml, re, sys, glob, io
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')
R36='E:/references/dart-sdk-3.6.2/pkg/'; RM='E:/references/dart-sdk/pkg/'
def load(p): return yaml.safe_load(open(p,encoding='utf-8'))
def snake(s): return ''.join(('_'+ch.lower()) if ch.isupper() else ch for ch in s).lstrip('_')
def norm(m):
    if m is None: return None
    m=re.sub(r'\{\d+\}','{}',m); m=re.sub(r'#[a-zA-Z]+[0-9]*','{}',m); return m
old={}  # snake -> list of (unique, pm, cm, nargs)
a=load(R36+'analyzer/messages.yaml')
for cls,d in a.items():
    for name,e in d.items():
        if not isinstance(e,dict): continue
        if 'problemMessage' not in e: continue
        code=(e.get('sharedName') or name).lower()
        pm=e['problemMessage']; cm=e.get('correctionMessage')
        n=len(set(re.findall(r'\{(\d+)\}',pm+(cm or ''))))
        old.setdefault(code,[]).append((cls+'.'+name,pm,cm,n))
f=load(R36+'front_end/messages.yaml')
for name,e in f.items():
    if isinstance(e,dict) and 'analyzerCode' in e and 'index' in e:
        code=str(e.get('sharedName') or e['analyzerCode']).split('.')[-1].lower()
        pm=e.get('problemMessage'); cm=e.get('correctionMessage')
        n=len(set(re.findall(r'#[a-zA-Z]+[0-9]*',(pm or '')+(cm or ''))))
        old.setdefault(code,[]).append(('fe:'+name,pm,cm,n))
new={}
a=load(RM+'analyzer/messages.yaml')
for cls,d in a.items():
    for name,e in d.items():
        if not isinstance(e,dict) or 'problemMessage' not in e: continue
        code=snake(e.get('sharedName') or name)
        params=e.get('parameters'); n=len(params) if isinstance(params,dict) else 0
        new.setdefault(code,[]).append((cls+'.'+name,e['problemMessage'],e.get('correctionMessage'),n,e.get('type'),params))
for fn in ['_fe_analyzer_shared/messages.yaml']:
    f=load(RM+fn)
    for name,e in f.items():
        if isinstance(e,dict) and 'analyzerCode' in e and 'problemMessage' in e:
            code=snake(str(e.get('sharedName') or e['analyzerCode']).split('.')[-1])
            params=e.get('parameters'); n=len(params) if isinstance(params,dict) else 0
            new.setdefault(code,[]).append((fn.split('/')[0]+':'+name,e['problemMessage'],e.get('correctionMessage'),n,e.get('type'),params))
codes=[]
for fam in 'ABCDEF':
    for l in open(f'E:/dftemp/analise/trab/familia-{fam}.txt',encoding='utf-8').read().splitlines()[1:]:
        if l.strip(): codes.append((fam,l.split()[0],l.split()[-1]))
same=0; out=[]
for fam,c,perda in codes:
    o=old.get(c); n=new.get(c)
    if o is None and n is None: out.append((fam,c,perda,'AUSENTE nas duas listas',[])); continue
    if o is None: out.append((fam,c,perda,'SO NO MAIN',[('main',x[0],x[1],x[2]) for x in n])); continue
    if n is None: out.append((fam,c,perda,'SO NO 3.6.2 (removido/renomeado no main)',[('3.6.2',x[0],x[1],x[2]) for x in o])); continue
    op={norm(x[1]) for x in o}; np_={norm(x[1]) for x in n}
    oc={norm(x[2]) for x in o}; nc={norm(x[2]) for x in n}
    on=sorted({x[3] for x in o}); nn=sorted({x[3] for x in n})
    ch=[]
    if op!=np_: ch.append('mensagem')
    if oc!=nc: ch.append('correcao')
    if on!=nn: ch.append(f'argumentos {on}->{nn}')
    if len(o)!=len(n): ch.append(f'variantes {len(o)}->{len(n)}')
    if not ch: same+=1; continue
    det=[('3.6.2',x[0],x[1],x[2]) for x in o if norm(x[1]) not in np_ or norm(x[2]) not in nc or 'argumentos' in ' '.join(ch)]+[('main',x[0],x[1],x[2]) for x in n if norm(x[1]) not in op or norm(x[2]) not in oc or 'argumentos' in ' '.join(ch)]
    out.append((fam,c,perda,', '.join(ch),det))
print('codigos',len(codes),'iguais',same,'diferentes',len(out))
for fam,c,perda,ch,det in out:
    print(f'\n[{fam}] {c} (perda {perda}): {ch}')
    for v,u,pm,cm in det: print(f'   {v} {u}\n      PM: {pm}\n      CM: {cm}')

import json,re,collections,io,sys
sys.stdout=io.TextIOWrapper(sys.stdout.buffer,encoding='utf-8')
base='E:/MyRustProjects/dartforge/corpus/diagnosticos/'
sint=set(); tipos=collections.Counter()
porarq=collections.defaultdict(list)
for g in ['analyzer','linguagem','analyzer-3.13','linguagem-3.13']:
    for l in open(base+g+'/oraculo.jsonl',encoding='utf-8'):
        r=json.loads(l); tipos[r['type']]+=1
        porarq[r['arquivo']].append(r)
        if r['type']=='SYNTACTIC_ERROR' and r['code'] not in ('experiment_not_enabled','experiment_not_enabled_off_by_default'): sint.add(r['arquivo'])
print(tipos, 'arquivos com erro sintatico', len(sint))
alvo=['unused_element','unused_field','unused_local_variable','unused_import','unused_catch_stack','unused_catch_clause','unused_shown_name','unused_label','undefined_identifier','undefined_function','undefined_method','undefined_getter','undefined_setter','undefined_operator','getter_not_subtype_setter_types','not_initialized_non_nullable_variable','dead_code','unused_element_parameter','duplicate_import','unnecessary_import']
cnt=collections.defaultdict(collections.Counter); ex=collections.defaultdict(list); tot=collections.Counter()
for l in open('E:/dftemp/analise/trab/placar-r7.txt',encoding='utf-8'):
    m=re.match(r"  \[(\w+)\] (FN|FP|mensagem|posição): (\S+?\.dart):(\d+):(\d+) (.*)",l)
    if not m: continue
    c,k,f=m.group(1),m.group(2),m.group(3)
    if c in alvo:
        tot[(c,k)]+=1
        if f in sint:
            cnt[c][k]+=1
            if len(ex[(c,k)])<4: ex[(c,k)].append(l.strip()[:200])
for c in alvo:
    if cnt[c]:
        print(c, dict(cnt[c]), 'de', {k:v for (cc,k),v in tot.items() if cc==c})
        for k in cnt[c]:
            for e in ex[(c,k)]: print('    ',e)

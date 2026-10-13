import re,sys,io,collections
sys.stdout=io.TextIOWrapper(sys.stdout.buffer,encoding='utf-8')
codes=sys.argv[2:]
full = sys.argv[1]=='full'
g=collections.defaultdict(lambda: collections.defaultdict(list))
for l in open(r'E:\dftemp\analise\trab\placar-r7.txt',encoding='utf-8',errors='replace'):
    m=re.match(r'\s+\[(\w+)\] (\S+): (\S+?):(\d+):(\d+) ?(.*)',l)
    if m and m.group(1) in codes:
        g[m.group(1)][(m.group(2),m.group(3))].append((int(m.group(4)),int(m.group(5)),m.group(6).strip()))
for c in codes:
    print('###',c)
    for (k,f),v in sorted(g[c].items()):
        if full:
            for x in v: print('  ',k,f,'%d:%d'%x[:2],x[2][:150])
        else:
            print('  ',k,f,len(v),' '.join('%d:%d'%x[:2] for x in v[:12]))

import json,sys,subprocess,os,io,collections
sys.stdout=io.TextIOWrapper(sys.stdout.buffer,encoding='utf-8')
env=dict(os.environ); env['ANALYZER_STATE_LOCATION_OVERRIDE']=r'E:\dftemp\analise\spec-r4\dartstate'
alvo=sys.argv[1]; filtro=sys.argv[2:] 
r=subprocess.run([r'C:\tools\dartsdk-3.6.2\bin\dart.exe','analyze','--format=json',alvo],capture_output=True,text=True,encoding='utf-8',env=env)
out=r.stdout; i=out.find('{"version"')
if i<0: print(out, r.stderr); sys.exit()
ds=json.loads(out[i:].split('\n')[0])['diagnostics']
g=collections.defaultdict(list)
for d in ds: g[os.path.basename(d['location']['file'])].append(d)
arqs=sorted(f for f in os.listdir(alvo) if f.endswith('.dart')) if os.path.isdir(alvo) else [os.path.basename(alvo)]
for f in arqs:
    if filtro and not any(f.startswith(x) for x in filtro): continue
    print('==',f)
    l=sorted(g.get(f,[]),key=lambda d:(d['location']['range']['start']['offset'],d['code']))
    if not l: print('  (nenhum diagnóstico)')
    for d in l:
        s=d['location']['range']['start']; e=d['location']['range']['end']
        print('  %s off=%d len=%d %d:%d | %s'%(d['code'],s['offset'],e['offset']-s['offset'],s['line'],s['column'],d['problemMessage']))
        for c in d.get('contextMessages',[]) or []:
            cs=c['location']['range']['start']; ce=c['location']['range']['end']
            print('      ctx off=%d len=%d | %s'%(cs['offset'],ce['offset']-cs['offset'],c['message']))

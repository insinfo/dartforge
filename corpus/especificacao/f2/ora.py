import json,sys,subprocess,os,io
sys.stdout=io.TextIOWrapper(sys.stdout.buffer,encoding='utf-8')
sdk={'36':'C:/tools/dartsdk-3.6.2/bin/dart.exe','313':'E:/DartSDKs/3.13.4/dart-sdk/bin/dart.exe'}[sys.argv[1]]
st={'36':'E:/dftemp/analise/spec-r4/dartstate','313':'E:/dftemp/analise/spec-r4/dartstate313'}[sys.argv[1]]
env=dict(os.environ); env['ANALYZER_STATE_LOCATION_OVERRIDE']=st.replace('/',os.sep)
here=os.path.dirname(os.path.abspath(__file__))
for f in sys.argv[2:]:
    r=subprocess.run([sdk,'analyze','--format=json',f],capture_output=True,text=True,encoding='utf-8',env=env)
    out=r.stdout
    i=out.find('{"version"')
    if i<0:
        print('==',f); print(out.strip(), r.stderr.strip()); continue
    ds=json.loads(out[i:].split('\n')[0])['diagnostics']
    by={}
    for d in ds:
        by.setdefault(os.path.normcase(os.path.abspath(d['location']['file'])),[]).append(d)
    if os.path.isfile(f):
        files=[f]
    else:
        files=sorted(os.path.join(dp,n) for dp,_,ns in os.walk(f) for n in ns if n.endswith('.dart'))
    for p in files:
        print('==',os.path.relpath(os.path.abspath(p),here).replace(os.sep,'/'))
        l=by.get(os.path.normcase(os.path.abspath(p)),[])
        l.sort(key=lambda d:(d['location']['range']['start']['offset'],d['code']))
        if not l: print('  (nenhum)')
        for d in l:
            s=d['location']['range']['start']; e=d['location']['range']['end']
            print('  %s | %d+%d | %d:%d | %s'%(d['code'],s['offset'],e['offset']-s['offset'],s['line'],s['column'],d['problemMessage']))

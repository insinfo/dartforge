import json,sys,subprocess,os,io
sys.stdout=io.TextIOWrapper(sys.stdout.buffer,encoding='utf-8')
env=dict(os.environ); env['ANALYZER_STATE_LOCATION_OVERRIDE']=r'E:\dftemp\analise\spec-r4\dartstate'
for f in sys.argv[1:]:
    r=subprocess.run([r'C:\tools\dartsdk-3.6.2\bin\dart.exe','analyze','--format=json',f],capture_output=True,text=True,encoding='utf-8',env=env)
    out=r.stdout
    i=out.find('{"version"')
    print('==',f)
    if i<0: print(out.strip(), r.stderr.strip()); continue
    ds=json.loads(out[i:].split('\n')[0])['diagnostics']
    ds.sort(key=lambda d:(d['location']['file'],d['location']['range']['start']['offset']))
    if not ds: print('  (nenhum diagnóstico)')
    for d in ds:
        s=d['location']['range']['start']; e=d['location']['range']['end']
        print('  %s | offset %d, length %d | %d:%d | %s | %s'%(d['code'],s['offset'],e['offset']-s['offset'],s['line'],s['column'],os.path.basename(d['location']['file']) if len(sys.argv)>2 and os.path.isdir(f) else d['severity'],d['problemMessage']))

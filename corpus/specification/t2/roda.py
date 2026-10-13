import json,subprocess,sys,os,io
sys.stdout=io.TextIOWrapper(sys.stdout.buffer,encoding='utf-8')
sdk={'36':'C:/tools/dartsdk-3.6.2/bin/dart.exe','313':'E:/DartSDKs/3.13.4/dart-sdk/bin/dart.exe'}[sys.argv[1]]
alvo=sys.argv[2]; corr='-c' in sys.argv
env=dict(os.environ); env['ANALYZER_STATE_LOCATION_OVERRIDE']='E:'+chr(92)+'dftemp'+chr(92)+'analise'+chr(92)+'spec-r4'+chr(92)+('dartstate' if sys.argv[1]=='36' else 'dartstate313'); env['DART_DISABLE_ANALYTICS']='1'
cwd=alvo if os.path.isdir(alvo) else os.path.dirname(os.path.abspath(alvo))
r=subprocess.run([sdk,'analyze','--format=json',os.path.abspath(alvo)],capture_output=True,cwd=cwd,env=env)
t=r.stdout.decode('utf-8','replace')
j=[l for l in t.splitlines() if l.startswith('{')]
if not j: print('SEM JSON',t[:500],r.stderr.decode('utf-8','replace')[:500]); sys.exit()
ds=json.loads(j[0])['diagnostics']
por={}
for d in ds:
    f=os.path.basename(d['location']['file']); por.setdefault(f,[]).append(d)
ordem=json.loads(j[0])['diagnostics']
if '-o' in sys.argv:
    for d in ordem:
        s=d['location']['range']['start']; print(os.path.basename(d['location']['file']),d['severity'],d['code'],f"{s['line']}:{s['column']}")
    sys.exit()
for f in sorted(por):
    print('--',f)
    for d in sorted(por[f],key=lambda d:(d['location']['range']['start']['offset'],d['code'])):
        s=d['location']['range']['start']; e=d['location']['range']['end']
        print(f"  {d['code']} [{d['severity']}] off {s['offset']} len {e['offset']-s['offset']} {s['line']}:{s['column']} | {d['problemMessage']}"+((' || '+str(d.get('correctionMessage'))) if corr else ''))

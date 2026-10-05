#!/bin/bash
# uso: all.sh [prefixo]  -> analisa a pasta inteira e imprime por arquivo
cd /e/dftemp/analise/spec-r4/casos/e2
ANALYZER_STATE_LOCATION_OVERRIDE='E:\dftemp\analise\spec-r4\dartstate' /c/tools/dartsdk-3.6.2/bin/dart analyze --format=json . 2>/dev/null > out.json
python - "$1" <<'PY'
import sys,json,os,glob
pre=sys.argv[1] if len(sys.argv)>1 else ''
t=open('out.json',encoding='utf-8').read()
i=t.find('{"version"')
d=json.loads(t[i:])
by={}
for x in d['diagnostics']:
    by.setdefault(os.path.basename(x['location']['file']),[]).append(x)
for f in sorted(glob.glob(pre+'*.dart')):
    print('### '+f)
    print(open(f,encoding='utf-8').read().rstrip('\n'))
    print('--- diag')
    ds=sorted(by.get(f,[]), key=lambda x:(x['location']['range']['start']['offset'], x['code']))
    for x in ds:
        r=x['location']['range']; s=r['start']; e=r['end']
        print('%s off=%d len=%d %d:%d | %s' % (x['code'], s['offset'], e['offset']-s['offset'], s['line'], s['column'], x['problemMessage']))
    if not ds: print('(nenhum)')
PY

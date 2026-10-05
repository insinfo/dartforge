#!/bin/bash
# uso: run.sh arquivo.dart ...   -> imprime diagnósticos compactos
cd /e/dftemp/analise/spec-r4/casos/e2
for f in "$@"; do
  echo "### $f"
  cat -A "$f" | sed 's/\$$//' | head -30
  echo "--- diag"
  ANALYZER_STATE_LOCATION_OVERRIDE='E:\dftemp\analise\spec-r4\dartstate' /c/tools/dartsdk-3.6.2/bin/dart analyze --format=json "$f" 2>/dev/null | python -c "
import sys,json
t=sys.stdin.read()
i=t.find('{\"version\"')
if i<0: print('(sem json)', t[:200]); sys.exit()
d=json.loads(t[i:])
ds=sorted(d['diagnostics'], key=lambda x:(x['location']['range']['start']['offset'], x['code']))
for x in ds:
    r=x['location']['range']; s=r['start']; e=r['end']
    print('%s off=%d len=%d %d:%d | %s' % (x['code'], s['offset'], e['offset']-s['offset'], s['line'], s['column'], x['problemMessage']))
if not ds: print('(nenhum)')
"
done

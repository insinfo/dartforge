import json, subprocess, sys, os, io
d = os.path.dirname(os.path.abspath(__file__))
env = dict(os.environ)
env['ANALYZER_STATE_LOCATION_OVERRIDE'] = r'E:\dftemp\analise\spec-r4\dartstate'
alvo = sys.argv[1] if len(sys.argv) > 1 else d
saida = sys.argv[2] if len(sys.argv) > 2 else os.path.join(d, 'saida.txt')
r = subprocess.run([r'C:\tools\dartsdk-3.6.2\bin\dart.exe', 'analyze', '--format=json', alvo], capture_output=True, env=env, cwd=d)
out = r.stdout.decode('utf8', errors='replace')
i = out.find('{"version"')
if i < 0:
    print(out[:2000]); print(r.stderr.decode('utf8', errors='replace')[:2000]); sys.exit(1)
j = json.loads(out[i:].splitlines()[0])
by = {}
for g in j['diagnostics']:
    f = os.path.basename(g['location']['file'])
    s = g['location']['range']['start']; e = g['location']['range']['end']
    by.setdefault(f, []).append((s['offset'], e['offset'] - s['offset'], s['line'], s['column'], g['code'], g['severity'], g['problemMessage']))
if os.path.isdir(alvo):
    names = sorted(f for f in os.listdir(alvo) if f.endswith('.dart'))
    base = alvo
else:
    names = [os.path.basename(alvo)]; base = os.path.dirname(alvo)
o = io.open(saida, 'w', encoding='utf8')
for f in names:
    src = io.open(os.path.join(base, f), encoding='utf8').read()
    o.write('=== %s\n%s\n' % (f, src.rstrip('\n')))
    for (off, ln, l, c, code, sev, msg) in sorted(by.get(f, [])):
        o.write('  %s @%d len %d (%d:%d) [%s] %s\n' % (code, off, ln, l, c, sev[0], msg))
o.close()

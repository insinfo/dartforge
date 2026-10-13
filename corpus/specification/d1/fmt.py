import json, sys, os
raw = sys.stdin.read()
i = raw.find('{"version"')
if i < 0:
    print(raw)
    sys.exit(0)
d = json.loads(raw[i:])
by = {}
for g in d['diagnostics']:
    f = os.path.basename(g['location']['file'])
    r = g['location']['range']
    s = "  %s off=%d len=%d %d:%d %s | %s" % (g['code'], r['start']['offset'], r['end']['offset'] - r['start']['offset'], r['start']['line'], r['start']['column'], g['severity'], g['problemMessage'])
    for c in g.get('contextMessages', []) or []:
        cr = c['location']['range']
        s += "\n      ctx off=%d len=%d %d:%d | %s" % (cr['start']['offset'], cr['end']['offset'] - cr['start']['offset'], cr['start']['line'], cr['start']['column'], c['message'])
    by.setdefault(f, []).append((r['start']['offset'], s))
for f in sorted(by):
    print(f)
    for _, s in sorted(by[f]):
        print(s)

import json, sys, os, collections

raw = open(sys.argv[1], encoding='utf8').read()
i = raw.find('{"version"')
d = json.loads(raw[i:])
by = collections.defaultdict(list)
for x in d['diagnostics']:
    l = x['location']
    r = l['range']
    f = os.path.basename(l['file'])
    by[f].append((r['start']['offset'], r['end']['offset'] - r['start']['offset'],
                  r['start']['line'], r['start']['column'], x['code'],
                  x['problemMessage'], x['severity']))


def key(n):
    b = os.path.splitext(n)[0]
    return (b[0], int(b[1:]) if b[1:].isdigit() else 0, b)


base = sys.argv[2] if len(sys.argv) > 2 else '.'
names = sorted(set(list(by.keys()) + [f for f in os.listdir(base) if f.endswith('.dart')]), key=key)
for f in names:
    p = os.path.join(base, f)
    src = open(p, encoding='utf8').read().rstrip('\n') if os.path.exists(p) else '?'
    print('###', f, '|', src.replace('\n', ' \\n '))
    for o, ln, li, co, c, m, s in sorted(by.get(f, [])):
        print(f'  {c} off={o} len={ln} {li}:{co} [{s[0]}] {m}')

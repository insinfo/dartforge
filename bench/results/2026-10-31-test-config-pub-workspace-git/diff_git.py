import json, os, glob
A = '/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad/ac4f'
S = os.path.dirname(A)
lock_a = open(f'{A}/gitsrc-official/app/pubspec.lock').read().split('\n', 2)[2]
lock_b = open(f'{A}/gitsrc-native/app/pubspec.lock').read().split('\n', 1)[1]
print('lock text identical apart from header:', lock_a == lock_b)
if lock_a != lock_b:
    print(lock_a)
    print(lock_b)


def config(path, cache):
    c = json.load(open(path))
    for k in ['generated', 'generator', 'generatorVersion', 'pubCache']:
        c.pop(k, None)
    for p in c['packages']:
        p['rootUri'] = p['rootUri'].replace('file://' + cache, '<cache>')
    return c


ca = config(f'{A}/gitsrc-official/app/.dart_tool/package_config.json', f'{A}/cache-official')
cb = config(f'{A}/gitsrc-native/app/.dart_tool/package_config.json', f'{S}/dartforge-pub-cache')
print('config equal:', ca == cb)
if ca != cb:
    print(json.dumps(ca, indent=1))
    print(json.dumps(cb, indent=1))
for cache in [f'{A}/cache-official', f'{S}/dartforge-pub-cache']:
    print(cache)
    print('  mirrors:', sorted(os.listdir(f'{cache}/git/cache')))
    for checkout in sorted(glob.glob(f'{cache}/git/tools_repo-*')):
        print('  ', os.path.basename(checkout), repr(open(f'{checkout}/.git/pub-packages').read()))

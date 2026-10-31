"""Compares `dart pub get` with `dartforge pub get` on two copies of a package.

usage: pubcmp.py <source dir> <work dir> <relative dir to run in> [--offline] [--with-lock]

Only pubspec.yaml, pubspec_overrides.yaml and (with --with-lock) pubspec.lock
files are copied; `lib/` directories are created empty. The produced
pubspec.lock files, package_config.json files (without generator fields) and
workspace_ref.json files must be equal.
"""
import json, os, shutil, subprocess, sys
import yaml

src, work, run_in = sys.argv[1], sys.argv[2], sys.argv[3]
offline = '--offline' in sys.argv
with_lock = '--with-lock' in sys.argv
DART = os.path.expanduser('~/tools/dart/3.6.2/dart-sdk/bin/dart')
DARTFORGE = '/Volumes/ssd_dock/MyRustProjects/dartforge/.claude/worktrees/agent-ac4f2e63b187e88ba/target/release/dartforge'
S = '/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad'
SKIP = {'.dart_tool', '.git', 'build', 'node_modules', '.idea', 'references', 'referencias'}


def copy(dst):
    shutil.rmtree(dst, ignore_errors=True)
    for dirpath, dirnames, filenames in os.walk(src):
        dirnames[:] = [d for d in dirnames if d not in SKIP]
        wanted = ['pubspec.yaml', 'pubspec_overrides.yaml'] + (['pubspec.lock'] if with_lock else [])
        for name in wanted:
            if name in filenames:
                rel = os.path.relpath(dirpath, src)
                os.makedirs(os.path.join(dst, rel, 'lib'), exist_ok=True)
                shutil.copy(os.path.join(dirpath, name), os.path.join(dst, rel, name))


def run(tool, dst):
    cmd = [DART if tool == 'dart' else DARTFORGE, 'pub', 'get'] + (['--offline'] if offline else [])
    env = dict(os.environ)
    if tool == 'dart':
        env['PUB_CACHE'] = '/Volumes/ssd_dock/tools/dart/pub-cache'
    else:
        env['PUB_CACHE'] = os.path.join(S, 'dartforge-pub-cache')
    r = subprocess.run(cmd, cwd=os.path.join(dst, run_in), env=env, capture_output=True, text=True, timeout=1800)
    return r


def outputs(dst):
    result = {}
    for dirpath, dirnames, filenames in os.walk(dst):
        rel = os.path.relpath(dirpath, dst)
        for name in filenames:
            path = os.path.join(dirpath, name)
            key = os.path.join(rel, name)
            if name == 'pubspec.lock':
                result[key] = ''.join(l for l in open(path) if not l.startswith('#'))
            elif name == 'package_config.json':
                config = json.load(open(path))
                for k in ['generated', 'generator', 'generatorVersion', 'pubCache']:
                    config.pop(k, None)
                for entry in config['packages']:
                    for cache in ['file:///Volumes/ssd_dock/tools/dart/pub-cache/', 'file://' + S + '/dartforge-pub-cache/']:
                        if entry['rootUri'].startswith(cache):
                            entry['rootUri'] = '<cache>/' + entry['rootUri'][len(cache):]
                result[key] = config
            elif name == 'workspace_ref.json':
                result[key] = open(path).read()
    return result


a, b = os.path.join(work, 'official'), os.path.join(work, 'native')
copy(a)
copy(b)
ra = run('dart', a)
rb = run('dartforge', b)
print('dart exit', ra.returncode, '| dartforge exit', rb.returncode)
if ra.returncode or rb.returncode:
    print('--- dart\n', ra.stdout[-3000:], ra.stderr[-3000:])
    print('--- dartforge\n', rb.stdout[-3000:], rb.stderr[-3000:])
oa, ob = outputs(a), outputs(b)
same = (ra.returncode == 0) == (rb.returncode == 0)
for key in sorted(set(oa) | set(ob)):
    if oa.get(key) != ob.get(key):
        same = False
        print('!! differs:', key)
        x, y = oa.get(key), ob.get(key)
        if isinstance(x, dict) and isinstance(y, dict) and 'packages' in x and 'packages' in y:
            px, py = x['packages'], y['packages']
            if isinstance(px, dict):
                for n in sorted(set(px) | set(py)):
                    if px.get(n) != py.get(n):
                        print('   ', n, px.get(n), '|', py.get(n))
                print('   sdks', x.get('sdks'), y.get('sdks'))
            else:
                print('   order dart     ', [p['name'] for p in px])
                print('   order dartforge', [p['name'] for p in py])
                dx = {p['name']: p for p in px}
                dy = {p['name']: p for p in py}
                for n in sorted(set(dx) | set(dy)):
                    if dx.get(n) != dy.get(n):
                        print('   ', n, dx.get(n), '|', dy.get(n))
        else:
            print('   dart     ', str(x)[:400])
            print('   dartforge', str(y)[:400])
    else:
        print('   same:', key)
print('SAME' if same else 'DIFFERENT')

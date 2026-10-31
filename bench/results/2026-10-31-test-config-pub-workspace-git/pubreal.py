"""Copies the pubspec files of /Volumes/ssd_dock/MyProjectsDart twice and
compares `dart pub get` (offline, real cache) with `dartforge pub get`
(scratch cache) in each listed project, keeping each project's lockfile.

usage: pubreal.py <work dir> <project>...
"""
import json, os, shutil, subprocess, sys
import yaml

SRC = '/Volumes/ssd_dock/MyProjectsDart'
S = '/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad'
DART = os.path.expanduser('~/tools/dart/3.6.2/dart-sdk/bin/dart')
DARTFORGE = '/Volumes/ssd_dock/MyRustProjects/dartforge/.claude/worktrees/agent-ac4f2e63b187e88ba/target/release/dartforge'
SKIP = {'.dart_tool', '.git', 'build', 'node_modules', '.idea', 'references', 'referencias', 'third_party'}
work, projects = sys.argv[1], sys.argv[2:]
offline_native = os.environ.get('NATIVE_OFFLINE') == '1'


def copy(dst):
    shutil.rmtree(dst, ignore_errors=True)
    for dirpath, dirnames, filenames in os.walk(SRC):
        dirnames[:] = [d for d in dirnames if d not in SKIP and not d.startswith('.')]
        for name in ['pubspec.yaml', 'pubspec_overrides.yaml', 'pubspec.lock']:
            if name in filenames:
                rel = os.path.relpath(dirpath, SRC)
                os.makedirs(os.path.join(dst, rel, 'lib'), exist_ok=True)
                shutil.copy(os.path.join(dirpath, name), os.path.join(dst, rel, name))


def outputs(dst):
    result = {}
    for dirpath, dirnames, filenames in os.walk(dst):
        rel = os.path.relpath(dirpath, dst)
        for name in filenames:
            path = os.path.join(dirpath, name)
            key = os.path.join(rel, name)
            if name == 'pubspec.lock':
                # Pub's text without its header comments.
                result[key] = ''.join(l for l in open(path) if not l.startswith('#'))
            elif name == 'package_config.json':
                config = json.load(open(path))
                for k in ['generated', 'generator', 'generatorVersion', 'pubCache']:
                    config.pop(k, None)
                for entry in config['packages']:
                    for cache in ['file://' + S + '/ac4f/cache-official/', 'file://' + S + '/dartforge-pub-cache/']:
                        if entry['rootUri'].startswith(cache):
                            entry['rootUri'] = '<cache>/' + entry['rootUri'][len(cache):]
                result[key] = config
            elif name == 'workspace_ref.json':
                result[key] = open(path).read()
    return result


official, native = os.path.join(work, 'official'), os.path.join(work, 'native')
copy(official)
copy(native)
summary = []
for project in projects:
    runs = {}
    for tool, root in [('dart', official), ('dartforge', native)]:
        env = dict(os.environ)
        cmd = [DART if tool == 'dart' else DARTFORGE, 'pub', 'get']
        if tool == 'dart':
            env['PUB_CACHE'] = os.path.join(S, 'ac4f', 'cache-official')
        else:
            env['PUB_CACHE'] = os.path.join(S, 'dartforge-pub-cache')
            if offline_native:
                cmd.append('--offline')
        r = subprocess.run(cmd, cwd=os.path.join(root, project), env=env, capture_output=True, text=True, timeout=3600)
        runs[tool] = r
    ra, rb = runs['dart'], runs['dartforge']
    pa, pb = os.path.join(official, project), os.path.join(native, project)
    # The workspace root of a member holds the outputs.
    oa = outputs(pa)
    ob = outputs(pb)
    keys = sorted(set(oa) | set(ob))
    diffs = [k for k in keys if oa.get(k) != ob.get(k)]
    status = 'SAME' if not diffs and (ra.returncode == 0) == (rb.returncode == 0) else 'DIFFERENT'
    if ra.returncode == 0 and rb.returncode != 0:
        status = 'DARTFORGE-FAILED'
    print(f'=== {project}: dart={ra.returncode} dartforge={rb.returncode} {status}')
    if rb.returncode:
        print('   dartforge:', (rb.stderr or rb.stdout).strip()[-600:])
    if ra.returncode:
        print('   dart:', (ra.stderr or ra.stdout).strip()[-600:])
    for k in diffs:
        x, y = oa.get(k), ob.get(k)
        print('   !! differs:', k)
        if isinstance(x, dict) and isinstance(y, dict):
            px, py = x.get('packages'), y.get('packages')
            if isinstance(px, dict) and isinstance(py, dict):
                for n in sorted(set(px) | set(py)):
                    if px.get(n) != py.get(n):
                        print('     ', n, px.get(n), '|', py.get(n))
                if x.get('sdks') != y.get('sdks'):
                    print('      sdks', x.get('sdks'), y.get('sdks'))
            elif isinstance(px, list) and isinstance(py, list):
                if [p['name'] for p in px] != [p['name'] for p in py]:
                    print('      order dart     ', [p['name'] for p in px])
                    print('      order dartforge', [p['name'] for p in py])
                dx = {p['name']: p for p in px}
                dy = {p['name']: p for p in py}
                for n in sorted(set(dx) | set(dy)):
                    if dx.get(n) != dy.get(n):
                        print('     ', n, dx.get(n), '|', dy.get(n))
        else:
            print('      ', str(x)[:200], '|', str(y)[:200])
    summary.append((project, status))
print('SUMMARY', summary)

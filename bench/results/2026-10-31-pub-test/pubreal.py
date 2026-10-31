"""Differential `dart pub <command>` vs `dartforge pub <command>` on copies
of the pubspec files of real projects.

usage: pubreal.py <work dir> <command args...> -- <project>...

Each project's pubspec.yaml / pubspec_overrides.yaml / pubspec.lock files
(KEEP_LOCK=0 drops the lockfiles) are copied twice, `lib/` directories are
created empty, and the command runs in both copies with separate caches
(DART_CACHE / DF_CACHE). Compared byte for byte: pubspec.lock,
package_config.json (without the `generated` timestamp, with the cache
prefix normalized), workspace_ref.json, pubspec.yaml (for editing
commands), stdout (cache prefix normalized) and the exit code.
"""
import json, os, re, shutil, subprocess, sys

SRC = os.environ.get('SRC', '/Volumes/ssd_dock/MyProjectsDart')
PT = '/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad/pt'
DART = '/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart'
DARTFORGE = os.environ.get('DARTFORGE', '/Volumes/ssd_dock/MyRustProjects/dartforge/.claude/worktrees/agent-a2e441933d35a0d49/target/release/dartforge')
DART_CACHE = os.environ.get('DART_CACHE', PT + '/cache-dart')
DF_CACHE = os.environ.get('DF_CACHE', PT + '/cache-df')
SKIP = {'.dart_tool', '.git', 'build', 'node_modules', '.idea', 'references', 'referencias', 'third_party'}
KEEP_LOCK = os.environ.get('KEEP_LOCK', '1') == '1'
COPY = os.environ.get('COPY', '1') == '1'

sep = sys.argv.index('--')
work, command, projects = sys.argv[1], sys.argv[2:sep], sys.argv[sep + 1:]


def copy(dst):
    shutil.rmtree(dst, ignore_errors=True)
    for dirpath, dirnames, filenames in os.walk(SRC):
        dirnames[:] = [d for d in dirnames if d not in SKIP and not d.startswith('.')]
        names = ['pubspec.yaml', 'pubspec_overrides.yaml'] + (['pubspec.lock'] if KEEP_LOCK else [])
        for name in names:
            if name in filenames:
                rel = os.path.relpath(dirpath, SRC)
                os.makedirs(os.path.join(dst, rel, 'lib'), exist_ok=True)
                shutil.copy(os.path.join(dirpath, name), os.path.join(dst, rel, name))


def normalize(text, cache):
    text = text.replace('file://' + cache, '<cache>').replace(cache, '<cache>')
    return text


def outputs(root, cache):
    result = {}
    for dirpath, dirnames, filenames in os.walk(root):
        rel = os.path.relpath(dirpath, root)
        for name in filenames:
            path = os.path.join(dirpath, name)
            key = os.path.join(rel, name)
            if name in ('pubspec.lock', 'pubspec.yaml', 'workspace_ref.json'):
                result[key] = open(path).read()
            elif name == 'package_config.json':
                text = re.sub(r'"generated": "[^"]*"', '"generated": "<time>"', open(path).read())
                result[key] = normalize(text, cache)
    return result


official, native = os.path.join(work, 'official'), os.path.join(work, 'native')
if COPY:
    copy(official)
    copy(native)
summary = []
for project in projects:
    runs = {}
    for tool, root in [('dart', official), ('dartforge', native)]:
        env = dict(os.environ)
        env['PUB_CACHE'] = DART_CACHE if tool == 'dart' else DF_CACHE
        cmd = [DART if tool == 'dart' else DARTFORGE, 'pub'] + command
        r = subprocess.run(cmd, cwd=os.path.join(root, project), env=env, capture_output=True, text=True, timeout=3600)
        runs[tool] = r
    ra, rb = runs['dart'], runs['dartforge']
    oa = outputs(os.path.join(official, project), DART_CACHE)
    ob = outputs(os.path.join(native, project), DF_CACHE)
    diffs = [k for k in sorted(set(oa) | set(ob)) if oa.get(k) != ob.get(k)]
    sa, sb = normalize(ra.stdout, DART_CACHE), normalize(rb.stdout, DF_CACHE)
    status = 'SAME'
    if diffs:
        status = 'FILES-DIFFER'
    elif ra.returncode != rb.returncode:
        status = 'EXIT-DIFFERS'
    elif sa != sb:
        status = 'STDOUT-DIFFERS'
    print(f'=== {project}: dart={ra.returncode} dartforge={rb.returncode} {status}')
    if rb.returncode or ra.returncode:
        print('   dart stderr:', ra.stderr.strip()[-1500:])
        print('   dartforge stderr:', rb.stderr.strip()[-1500:])
    if sa != sb:
        print('   --- dart stdout\n' + sa[-3000:])
        print('   --- dartforge stdout\n' + sb[-3000:])
    for k in diffs:
        print('   !! differs:', k)
        a, b = (oa.get(k) or '').splitlines(), (ob.get(k) or '').splitlines()
        import difflib
        for line in list(difflib.unified_diff(a, b, 'dart', 'dartforge', lineterm='', n=1))[:40]:
            print('      ' + line)
    summary.append((project, status))
print('SUMMARY', summary)
print('COUNTS', {s: sum(1 for _, x in summary if x == s) for s in set(x for _, x in summary)})

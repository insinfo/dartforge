"""Runs `dart test` and `dartforge test` with the same arguments and compares
the per-test outcomes and exit codes.

usage: compare.py <fixture> [args...]
"""
import json, os, subprocess, sys

S = '/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad'
DART = os.path.expanduser('~/tools/dart/3.6.2/dart-sdk/bin/dart')
DARTFORGE = '/Volumes/ssd_dock/MyRustProjects/dartforge/.claude/worktrees/agent-ac4f2e63b187e88ba/target/release/dartforge'
fixture, args = sys.argv[1], sys.argv[2:]
env = dict(os.environ)
env['PUB_CACHE'] = '/Volumes/ssd_dock/tools/dart/pub-cache'
env['PATH'] = '/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin:' + env['PATH']


def official():
    p = subprocess.run([DART, 'test', '-r', 'json', *args], cwd=fixture, env=env,
                       capture_output=True, text=True, timeout=900)
    tests, suites, out = {}, {}, {}
    for line in p.stdout.splitlines():
        try:
            e = json.loads(line)
        except ValueError:
            continue
        if e['type'] == 'suite':
            suites[e['suite']['id']] = e['suite']['path']
        if e['type'] == 'testStart':
            tests[e['test']['id']] = e['test']
        if e['type'] == 'testDone':
            t = tests[e['testID']]
            name = t['name']
            if e['hidden'] and e['result'] == 'success':
                continue
            if name.startswith('loading '):
                name = 'loading'
            result = 'skipped' if e['skipped'] else e['result']
            out.setdefault((suites[t['suiteID']], name), []).append(result)
    return p.returncode, out, p.stderr


def native():
    p = subprocess.run([DARTFORGE, 'test', '--sdk',
                        os.path.expanduser('~/tools/dart/3.6.2/dart-sdk/lib'), '-r', 'json', *args],
                       cwd=fixture, env=env, capture_output=True, text=True, timeout=1800)
    out = {}
    for line in p.stdout.splitlines():
        try:
            e = json.loads(line)
        except ValueError:
            continue
        if e['type'] == 'testDone':
            result = e['result']
            if result == 'failure':
                result = 'failure'
            out.setdefault((e['suite'], e['name']), []).append(result)
        if e['type'] == 'loadError':
            out.setdefault((e['suite'], 'loading'), []).append('error')
    return p.returncode, out, p.stderr


a = official()
b = native()
print('dart exit', a[0], 'dartforge exit', b[0])
keys = sorted(set(a[1]) | set(b[1]))
same = a[0] == b[0]
for k in keys:
    x, y = a[1].get(k), b[1].get(k)
    mark = '  ' if x == y else '!!'
    same &= x == y
    print(mark, k, x, y)
if not same:
    print('--- dart stderr\n' + a[2][-2000:])
    print('--- dartforge stderr\n' + b[2][-2000:])
print('SAME' if same else 'DIFFERENT')

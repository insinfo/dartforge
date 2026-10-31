import os, subprocess, shutil, sys
S = '/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad'
DART = os.path.expanduser('~/tools/dart/3.6.2/dart-sdk/bin/dart')
env = dict(os.environ, PUB_CACHE='/Volumes/ssd_dock/tools/dart/pub-cache')


def make(root, files):
    shutil.rmtree(root, ignore_errors=True)
    for rel, text in files.items():
        p = os.path.join(root, rel)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        open(p, 'w').write(text)


def get(cwd):
    r = subprocess.run([DART, 'pub', 'get', '--offline'], cwd=cwd, env=env, capture_output=True, text=True)
    print('exit', r.returncode)
    print(r.stdout[-1500:], r.stderr[-1500:])


base_root = "name: ws_root\nenvironment:\n  sdk: ^3.6.0\nworkspace:\n  - pkgs/a\n  - pkgs/b\n"
b = "name: b\nversion: 0.2.0\nenvironment:\n  sdk: ^3.5.0\nresolution: workspace\n"
variants = {
    'v1-member-overrides': {'pkgs/a/pubspec.yaml': "name: a\nenvironment:\n  sdk: ^3.6.0\nresolution: workspace\ndependencies:\n  meta: ^1.15.0\ndependency_overrides:\n  meta: 1.15.0\n", 'pkgs/b/pubspec.yaml': b},
    'v2-incompatible-sibling': {'pkgs/a/pubspec.yaml': "name: a\nenvironment:\n  sdk: ^3.6.0\nresolution: workspace\ndependencies:\n  b: ^9.0.0\n", 'pkgs/b/pubspec.yaml': b},
    'v3-path-sibling': {'pkgs/a/pubspec.yaml': "name: a\nenvironment:\n  sdk: ^3.6.0\nresolution: workspace\ndependencies:\n  b:\n    path: ../b\n", 'pkgs/b/pubspec.yaml': b},
    'v4-stale-member-files': {'pkgs/a/pubspec.yaml': "name: a\nenvironment:\n  sdk: ^3.6.0\nresolution: workspace\n", 'pkgs/a/pubspec.lock': "packages: {}\nsdks:\n  dart: any\n", 'pkgs/a/.dart_tool/package_config.json': '{"configVersion":2,"packages":[]}', 'pkgs/b/pubspec.yaml': b},
    'v6-unlisted': {'pkgs/a/pubspec.yaml': "name: a\nenvironment:\n  sdk: ^3.6.0\nresolution: workspace\n", 'pkgs/b/pubspec.yaml': b, 'pkgs/c/pubspec.yaml': "name: c\nenvironment:\n  sdk: ^3.6.0\nresolution: workspace\n"},
    'v8-old-sdk-member': {'pkgs/a/pubspec.yaml': "name: a\nenvironment:\n  sdk: ^3.4.0\nresolution: workspace\n", 'pkgs/b/pubspec.yaml': b},
    'v9-root-overrides-member-dep': {'pubspec.yaml': base_root + "dependency_overrides:\n  meta: 1.15.0\n", 'pkgs/a/pubspec.yaml': "name: a\nenvironment:\n  sdk: ^3.6.0\nresolution: workspace\ndependencies:\n  meta: ^1.16.0\n", 'pkgs/b/pubspec.yaml': b},
}
which = sys.argv[1:] or list(variants)
for name in which:
    root = os.path.join(S, 'wsprobe', name)
    files = {'pubspec.yaml': base_root}
    files.update(variants[name])
    make(root, files)
    print('=====', name)
    cwd = os.path.join(root, 'pkgs/c' if name == 'v6-unlisted' else 'pkgs/a')
    get(cwd)
    for dirpath, _, names in os.walk(root):
        for n in names:
            print('  ', os.path.relpath(os.path.join(dirpath, n), root))
# v7: plain package
root = os.path.join(S, 'wsprobe', 'v7-plain')
make(root, {'pubspec.yaml': "name: plain\nenvironment:\n  sdk: ^3.6.0\n"})
print('===== v7-plain')
get(root)
for dirpath, _, names in os.walk(root):
    for n in names:
        print('  ', os.path.relpath(os.path.join(dirpath, n), root))

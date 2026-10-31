import os, subprocess, shutil
S = '/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad/exprobe'
DART = os.path.expanduser('~/tools/dart/3.6.2/dart-sdk/bin/dart')
env = dict(os.environ, PUB_CACHE='/Volumes/ssd_dock/tools/dart/pub-cache')
for variant, extra in [('plain', []), ('noexample', ['--no-example']), ('enforce-missing', ['--enforce-lockfile']), ('dry', ['--dry-run'])]:
    root = os.path.join(S, variant)
    shutil.rmtree(root, ignore_errors=True)
    files = {
        'pubspec.yaml': "name: pkg\nenvironment:\n  sdk: ^3.6.0\n",
        'example/pubspec.yaml': "name: pkg_example\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  pkg:\n    path: ..\n",
    }
    for rel, text in files.items():
        p = os.path.join(root, rel)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        open(p, 'w').write(text)
    r = subprocess.run([DART, 'pub', 'get', '--offline', *extra], cwd=root, env=env, capture_output=True, text=True)
    print('=====', variant, r.returncode)
    print(r.stdout, r.stderr)
    for dirpath, _, names in os.walk(root):
        for n in names:
            print('  ', os.path.relpath(os.path.join(dirpath, n), root))
# enforce with a stale lock
root = os.path.join(S, 'plain')
open(os.path.join(root, 'pubspec.yaml'), 'w').write("name: pkg\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  meta: ^1.15.0\n")
r = subprocess.run([DART, 'pub', 'get', '--offline', '--enforce-lockfile'], cwd=root, env=env, capture_output=True, text=True)
print('===== enforce-stale', r.returncode, r.stdout, r.stderr)
print(open(os.path.join(root, 'pubspec.lock')).read())
print(open(os.path.join(root, 'example/pubspec.lock')).read())
print(open(os.path.join(root, 'example/.dart_tool/package_config.json')).read())

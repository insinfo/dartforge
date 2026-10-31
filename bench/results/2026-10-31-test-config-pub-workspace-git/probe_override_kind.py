import os, subprocess, shutil
S = '/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad/kindprobe'
DART = os.path.expanduser('~/tools/dart/3.6.2/dart-sdk/bin/dart')
shutil.rmtree(S, ignore_errors=True)
files = {
    'app/pubspec.yaml': "name: app\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  a:\n    path: ../a\n  b: ^1.0.0\ndev_dependencies:\n  c:\n    path: ../c\ndependency_overrides:\n  b:\n    path: ../b\n  c:\n    path: ../c\n  extra:\n    path: ../extra\n",
    'a/pubspec.yaml': "name: a\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  b: ^1.0.0\n",
    'b/pubspec.yaml': "name: b\nversion: 2.0.0\nenvironment:\n  sdk: ^3.6.0\n",
    'c/pubspec.yaml': "name: c\nenvironment:\n  sdk: ^3.6.0\n",
    'extra/pubspec.yaml': "name: extra\nenvironment:\n  sdk: ^3.6.0\n",
}
for rel, text in files.items():
    p = os.path.join(S, rel)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    open(p, 'w').write(text)
r = subprocess.run([DART, 'pub', 'get', '--offline'], cwd=os.path.join(S, 'app'),
                   env=dict(os.environ, PUB_CACHE='/Volumes/ssd_dock/tools/dart/pub-cache'), capture_output=True, text=True)
print(r.returncode, r.stdout, r.stderr)
print(open(os.path.join(S, 'app/pubspec.lock')).read())
print(open(os.path.join(S, 'app/.dart_tool/package_config.json')).read())

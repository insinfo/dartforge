import os, subprocess, sys, shutil
root = sys.argv[1]
shutil.rmtree(root, ignore_errors=True)
files = {
    'pubspec.yaml': "name: ws_root\nenvironment:\n  sdk: ^3.6.0\nworkspace:\n  - pkgs/a\n  - pkgs/b\ndev_dependencies:\n  lints: ^5.0.0\n",
    'pkgs/a/pubspec.yaml': "name: a\nversion: 1.0.0\nenvironment:\n  sdk: ^3.6.0\nresolution: workspace\ndependencies:\n  b: any\n  collection: ^1.18.0\n  local:\n    path: ../../local\n",
    'pkgs/a/lib/a.dart': "int a = 1;\n",
    'pkgs/b/pubspec.yaml': "name: b\nversion: 0.2.0\nenvironment:\n  sdk: ^3.5.0\nresolution: workspace\ndependencies:\n  meta: ^1.15.0\ndev_dependencies:\n  path: ^1.9.0\n",
    'pkgs/b/lib/b.dart': "int b = 2;\n",
    'local/pubspec.yaml': "name: local\nversion: 3.0.0\nenvironment:\n  sdk: ^3.0.0\ndependencies:\n  collection: any\n",
    'local/lib/local.dart': "int l = 3;\n",
}
for rel, text in files.items():
    p = os.path.join(root, rel)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    open(p, 'w').write(text)
print('ok')

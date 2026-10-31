import os, subprocess
S = '/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad/fixtures'
DART = os.path.expanduser('~/tools/dart/3.6.2/dart-sdk/bin/dart')


def package(name, files, test_version='1.25.7'):
    root = os.path.join(S, name)
    for rel, text in files.items():
        path = os.path.join(root, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, 'w') as f:
            f.write(text)
    with open(os.path.join(root, 'pubspec.yaml'), 'w') as f:
        f.write(f"name: fixture_{name.replace('-', '_')}\nenvironment:\n  sdk: '>=3.6.0 <4.0.0'\n"
                f"dev_dependencies:\n  test: {test_version}\n")
    env = dict(os.environ, PUB_CACHE='/Volumes/ssd_dock/tools/dart/pub-cache')
    r = subprocess.run([DART, 'pub', 'get', '--offline'], cwd=root, env=env,
                       capture_output=True, text=True)
    print(name, r.returncode, r.stderr[-300:])


simple = "import 'package:test/test.dart';\nvoid main() { test('fine', () {}); }\n"
package('errors', {
    'test/double_timeout_test.dart': "@Timeout(Duration(seconds: 1))\n@Timeout.none\nlibrary;\nimport 'package:test/test.dart';\nvoid main() { test('a', () {}); }\n",
    'test/bad_selector_test.dart': "@TestOn('vm && bogus')\nlibrary;\nimport 'package:test/test.dart';\nvoid main() { test('a', () {}); }\n",
    'test/bad_tag_test.dart': "@Tags(['ok', 'not ok'])\nlibrary;\nimport 'package:test/test.dart';\nvoid main() { test('a', () {}); }\n",
    'test/unknown_tag_test.dart': "@Tags(['undeclared'])\nlibrary;\nimport 'package:test/test.dart';\nvoid main() { test('a', () {}, tags: 'other'); }\n",
    'test/fine_test.dart': simple,
})
package('yaml-chrome', {'dart_test.yaml': 'platforms: [chrome]\n', 'test/a_test.dart': simple})
package('yaml-invalid', {'dart_test.yaml': 'timeout: abc\n', 'test/a_test.dart': simple})
package('yaml-preset', {'dart_test.yaml': 'add_presets: [missing]\n', 'test/a_test.dart': simple})
package('yaml-github', {'dart_test.yaml': 'reporter: github\n', 'test/a_test.dart': simple})
package('yaml-include', {
    'dart_test.yaml': 'include: base.yaml\npresets:\n  quick:\n    paths: [quick]\n',
    'base.yaml': 'filename: "*_spec.dart"\ntags:\n  db:\n    timeout: 100ms\n',
    'test/a_spec.dart': "@Tags(['db'])\nlibrary;\nimport 'package:test/test.dart';\nvoid main() { test('slow db', () => Future<void>.delayed(const Duration(milliseconds: 500))); }\n",
    'test/b_test.dart': simple,
    'quick/c_spec.dart': simple,
})
package('latest', {
    'dart_test.yaml': 'tags:\n  slow:\n    skip: true\nretry: 1\n',
    'test/a_test.dart': "@Tags(['slow'])\nlibrary;\nimport 'package:test/test.dart';\nvoid main() { test('slow', () {}); }\n",
    'test/b_test.dart': "@Timeout(Duration(milliseconds: 200))\nlibrary;\nimport 'package:test/test.dart';\nvar n = 0;\nvoid main() { test('retry', () { n++; expect(n, 2); }); test('late', () => Future<void>.delayed(const Duration(seconds: 1))); }\n",
}, test_version='1.26.3')

#!/bin/bash
# A small package:test fixture and `dart test` / `dartforge test` runs on it.
PT=/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad/pt
DART=/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart
DF=/Volumes/ssd_dock/MyRustProjects/dartforge/.claude/worktrees/agent-a2e441933d35a0d49/target/release/dartforge
T=$PT/tst
export PATH=/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin:$PATH
if [ "$1" = setup ]; then
  rm -rf "$T"; mkdir -p "$T/test"
  printf 'name: tst\nenvironment:\n  sdk: ^3.5.0\ndev_dependencies:\n  test: 1.25.7\n' > "$T/pubspec.yaml"
  printf "import 'package:test/test.dart';\n\nvoid main() {\n  test('passes', () => expect(1, 1));\n  group('g', () {\n    test('fails', () => expect(1, 2));\n    test('skipped', () {}, skip: 'not now');\n  });\n  test('prints', () => print('hello'));\n}\n" > "$T/test/a_test.dart"
  printf "import 'package:test/test.dart';\n\nvoid main() {\n  test('b passes', () {});\n}\n" > "$T/test/b_test.dart"
  (cd "$T" && PUB_CACHE=$PT/cache-dart "$DART" pub get > /dev/null)
  shift
fi
cd "$T"
echo "### dart test $*"
PUB_CACHE=$PT/cache-dart "$DART" test "$@" > "$PT/tst-dart.txt" 2>&1; echo "exit $?"
cat "$PT/tst-dart.txt"
if [ -x "$DF" ] && [ "$NODF" != 1 ]; then
  echo "### dartforge test $*"
  PUB_CACHE=$PT/cache-dart DARTFORGE_SDK_LIB=/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib "$DF" test "$@" > "$PT/tst-df.txt" 2>&1; echo "exit $?"
  cat "$PT/tst-df.txt"
fi

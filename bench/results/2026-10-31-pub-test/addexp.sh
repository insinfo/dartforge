#!/bin/bash
# Observes how `dart pub add/remove` edit pubspec.yaml.
PT=/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad/pt
DART=/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart
export PUB_CACHE=$PT/cache-dart
run() { # dir args...
  d=$1; shift
  echo "##### $d: pub $*"
  (cd "$PT/addexp/$d" && "$DART" pub "$@" > out.txt 2> err.txt; echo "exit $?"; cat out.txt; echo "--stderr"; cat err.txt; echo "--pubspec"; cat pubspec.yaml)
}
mk() { mkdir -p "$PT/addexp/$1"; printf "$2" > "$PT/addexp/$1/pubspec.yaml"; mkdir -p "$PT/addexp/$1/lib"; }
rm -rf "$PT/addexp"
mk sorted 'name: app\nenvironment:\n  sdk: ^3.5.0\n\ndependencies:\n  args: ^2.0.0\n  path: ^1.8.0 # comment\n\ndev_dependencies:\n  lints: ^5.0.0\n'
run sorted add meta
run sorted add "collection:>=1.17.0 <2.0.0"
run sorted add dev:test_api
run sorted add "yaml:{path: ../yamlpkg}"
mk unsorted 'name: app\nenvironment:\n  sdk: ^3.5.0\ndependencies:\n  path: ^1.8.0\n  args: ^2.0.0\n'
run unsorted add meta
run unsorted add args:2.4.0
mk nodeps 'name: app\nenvironment:\n  sdk: ^3.5.0\n'
run nodeps add path
run nodeps add --dev lints
mk gitadd 'name: app\nenvironment:\n  sdk: ^3.5.0\ndependencies:\n  path: any\n'
run gitadd add "pdfbox_dart:{git: https://github.com/insinfo/pdfbox_dart}"
run gitadd add "pdf_x:{git: {url: https://github.com/insinfo/pdfbox_dart, ref: main}}"
mk remove1 'name: app\nenvironment:\n  sdk: ^3.5.0\ndependencies:\n  path: ^1.8.0\n  args: ^2.0.0\n\ndev_dependencies:\n  lints: ^5.0.0\n'
run remove1 remove args
run remove1 remove lints
run remove1 remove nothere
run remove1 add dev:path
mk empty 'name: app\nenvironment:\n  sdk: ^3.5.0\ndependencies:\n'
run empty add path
mk flow 'name: app\nenvironment:\n  sdk: ^3.5.0\ndependencies: {}\n'
run flow add path
mk override 'name: app\nenvironment:\n  sdk: ^3.5.0\ndependencies:\n  path: ^1.8.0\n'
run override add "override:path:1.8.0"
run override add "nonexistent_pkg_dartforge_xyz"
run override add "path:^99.0.0"

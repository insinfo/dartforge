#!/bin/bash
# Differential pub add/remove/upgrade/deps cases: each case is a pubspec and
# a sequence of commands run in two copies (dart / dartforge).
PT=/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad/pt
W=$PT/addcmp
rm -rf "$W"
mk() { for side in a b; do mkdir -p "$W/$1/$side/lib"; printf "$2" > "$W/$1/$side/pubspec.yaml"; done; }
run() { c=$1; shift; echo "##### $c: pub $*"; bash "$PT/both.sh" "$W/$c/a" "$W/$c/b" "$@"; }
mk sorted 'name: app\nenvironment:\n  sdk: ^3.5.0\n\ndependencies:\n  args: ^2.0.0\n  path: ^1.8.0 # comment\n\ndev_dependencies:\n  lints: ^5.0.0\n'
run sorted add meta
run sorted add "collection:>=1.17.0 <2.0.0"
run sorted add dev:test_api
run sorted add "yaml:{path: ../yamlpkg}"
run sorted deps
run sorted deps --style=compact
run sorted deps --style=list
run sorted deps --no-dev
run sorted deps --json
run sorted deps --executables
mk unsorted 'name: app\nenvironment:\n  sdk: ^3.5.0\ndependencies:\n  path: ^1.8.0\n  args: ^2.0.0\n'
run unsorted add meta
run unsorted add args:2.4.0
run unsorted upgrade --major-versions
run unsorted downgrade
run unsorted upgrade path
run unsorted upgrade --tighten --dry-run
mk nodeps 'name: app\nenvironment:\n  sdk: ^3.5.0\n'
run nodeps add path
run nodeps add --dev lints
run nodeps add "meta:{hosted: https://pub.dev}"
mk gitadd 'name: app\nenvironment:\n  sdk: ^3.5.0\ndependencies:\n  path: any\n'
run gitadd add "pdfbox_dart:{git: https://github.com/insinfo/pdfbox_dart}"
mk remove1 'name: app\nenvironment:\n  sdk: ^3.5.0\ndependencies:\n  path: ^1.8.0\n  args: ^2.0.0\n\ndev_dependencies:\n  lints: ^5.0.0\n'
run remove1 get
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
mk conflict 'name: app\nenvironment:\n  sdk: ^3.5.0\ndependencies:\n  test: 1.25.0\n  test_api: 0.7.7\n'
run conflict get
mk oldsdk 'name: app\nenvironment:\n  sdk: ^3.5.0\ndependencies:\n  analyzer: 6.0.0\n'
run oldsdk get

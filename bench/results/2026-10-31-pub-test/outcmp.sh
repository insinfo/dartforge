#!/bin/bash
# Differential `pub outdated` cases.
PT=/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad/pt
W=$PT/outcmp
rm -rf "$W"
mk() { for side in a b; do mkdir -p "$W/$1/$side/lib"; printf "$2" > "$W/$1/$side/pubspec.yaml"; done; }
run() { c=$1; shift; echo "##### $c: pub $*"; bash "$PT/both.sh" "$W/$c/a" "$W/$c/b" "$@"; }
mk o1 'name: app\nenvironment:\n  sdk: ^3.5.0\ndependencies:\n  args: 2.4.0\n  path: ^1.8.0\n  http: ^0.13.0\ndev_dependencies:\n  lints: ^3.0.0\n'
run o1 outdated
run o1 get
run o1 outdated
run o1 outdated --json
run o1 outdated --no-dev-dependencies
run o1 outdated --show-all --no-transitive
run o1 outdated --prereleases
mk o2 'name: app\nenvironment:\n  sdk: ^3.5.0\ndependencies:\n  path: ^1.9.0\n'
run o2 get
run o2 outdated
run o2 outdated --json

#!/bin/zsh
S=/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad
cd $S
c() { echo "=== $*"; python3 -I compare.py "$@"; }
c fixtures/config
c fixtures/config -P ci
c fixtures/config -t quick
c fixtures/config -x slow -x quick
c fixtures/config -n 'library timeout$'
c fixtures/config -N plain
c fixtures/config --run-skipped
c fixtures/config --timeout 10x test/timeout_check.dart
c fixtures/config test/timeout_check.dart other/platform_check.dart test/ignored_test.dart
c fixtures/config test/nope_check.dart test/timeout_check.dart
c fixtures/config --total-shards 2 --shard-index 1
c fixtures/config --no-retry -j 1
c fixtures/config 'test/tagged_check.dart?name=plain'
c fixtures/config -n nothing-matches
c fixtures/config -t nothing-tagged
c fixtures/errors
c fixtures/errors test/unknown_tag_test.dart
c fixtures/yaml-invalid
c fixtures/yaml-preset
c fixtures/yaml-include
c fixtures/yaml-include -P quick
c fixtures/latest

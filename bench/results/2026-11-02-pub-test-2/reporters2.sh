#!/bin/bash
# Second batch of `dart test` / `dartforge test` scenarios.
. "$(dirname "$0")/env.sh"
T=$PT/tst
export DARTFORGE_TEST_COMPILE_JOBS=${DARTFORGE_TEST_COMPILE_JOBS:-2}
run() { tag=$1; shift; echo "== $tag: $*"; bash "$SP/bin/cmp.sh" "$T" "$tag" "$@"; }
# File reporter: each tool writes its own file.
mkdir -p "$SP/cmp/file-reporter"
(cd "$T" && PUB_CACHE=$PT/cache-dart "$DART" test -r compact -j 1 --file-reporter "json:$SP/cmp/file-reporter/dart.json" > "$SP/cmp/file-reporter/dart.out" 2> "$SP/cmp/file-reporter/dart.err"; echo $? > "$SP/cmp/file-reporter/dart.code")
(cd "$T" && PUB_CACHE=$PT/cache-dart "$DF" test -r compact -j 1 --file-reporter "json:$SP/cmp/file-reporter/df.json" > "$SP/cmp/file-reporter/df.out" 2> "$SP/cmp/file-reporter/df.err"; echo $? > "$SP/cmp/file-reporter/df.code")
echo "== file-reporter: exit dart $(cat "$SP/cmp/file-reporter/dart.code") dartforge $(cat "$SP/cmp/file-reporter/df.code")"
run fail-fast-json -r json -j 1 --fail-fast
run github-ff -r github -j 1 --fail-fast test/a_test.dart test/b_test.dart
run help --help
run version --version
run watch --watch
run coverage --coverage out
run seed -r json -j 1 --test-randomize-ordering-seed 42 test/a_test.dart
run tags -r expanded -j 1 -t slow

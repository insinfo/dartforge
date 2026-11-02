#!/bin/bash
# Runs every reporter scenario on the fixture with both tools.
. "$(dirname "$0")/env.sh"
T=$PT/tst
export DARTFORGE_TEST_COMPILE_JOBS=${DARTFORGE_TEST_COMPILE_JOBS:-2}
run() { tag=$1; shift; echo "== $tag: $*"; bash "$SP/bin/cmp.sh" "$T" "$tag" "$@"; }
run json -r json -j 1
run json-file -r expanded -j 1 --file-reporter json:$SP/cmp/json-file.json
run expanded -r expanded -j 1
run compact -r compact -j 1
run github -r github -j 1
run failures-only -r failures-only -j 1
run silent -r silent -j 1
run fail-fast -r expanded -j 1 --fail-fast
run json-parallel -r json
run name -r json -j 1 -N passes
run none -r json -j 1 -n nomatch
run badreporter -r nope

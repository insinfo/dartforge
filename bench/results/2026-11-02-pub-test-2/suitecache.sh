#!/bin/bash
# Runs `dartforge test -r json -j 1` three times on the fixture: with an empty
# suite cache, again (every suite from the cache) and after touching one suite
# (that change invalidates every key), timing each run and comparing each event
# stream with `dart test -r json -j 1` (`cmp/json`).
. "$(dirname "$0")/env.sh"
T=$PT/tst
export DARTFORGE_TEST_COMPILE_JOBS=2
rm -rf "$T/.dart_tool/dartforge/test-suites"
for run in cold warm touched; do
  [ "$run" = touched ] && touch "$T/test/b_test.dart"
  start=$(date +%s)
  (cd "$T" && PUB_CACHE=$PT/cache-dart "$DF" test -r json -j 1 > "$SP/cmp/json/df-$run.out" 2> "$SP/cmp/json/df-$run.err"; echo $? > "$SP/cmp/json/df-$run.code")
  end=$(date +%s)
  python3 "$SP/bin/jsonnorm.py" "$SP/cmp/json/df-$run.out" > "$SP/cmp/json/df-$run.n"
  python3 "$SP/bin/jsonnorm.py" "$SP/cmp/json/dart.out" > "$SP/cmp/json/dart.n"
  if diff -q "$SP/cmp/json/dart.n" "$SP/cmp/json/df-$run.n" > /dev/null; then same=identical; else same=DIFFERENT; fi
  echo "$run: $((end - start)) s, exit $(cat "$SP/cmp/json/df-$run.code"), events $same, cache entries $(ls "$T/.dart_tool/dartforge/test-suites" 2>/dev/null | wc -l | tr -d ' ')"
done

#!/bin/bash
# usage: cmp.sh <project dir> <tag> <test args...>
# Runs `dart test` and `dartforge test` with the same arguments in the project,
# keeping stdout, stderr and the exit code of each under $SP/cmp/<tag>.
. "$(dirname "$0")/env.sh"
dir=$1; tag=$2; shift 2
out=$SP/cmp/$tag
rm -rf "$out"; mkdir -p "$out"
cd "$dir" || exit 1
PUB_CACHE=$PT/cache-dart "$DART" test "$@" > "$out/dart.out" 2> "$out/dart.err"; echo $? > "$out/dart.code"
PUB_CACHE=$PT/cache-dart "$DF" test "$@" > "$out/df.out" 2> "$out/df.err"; echo $? > "$out/df.code"
echo "exit dart $(cat "$out/dart.code") dartforge $(cat "$out/df.code")"

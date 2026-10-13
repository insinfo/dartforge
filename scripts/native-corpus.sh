#!/usr/bin/env bash
# Runs corpus/native against `dart run`, several programs at a time.
#
#   scripts/native-corpus.sh [tracing|arc|prepared|tables] [jobs] [filter]
#
# `prepared` sets DARTFORGE_ARC_PREPARED=1 (program and SDK prepared);
# `tables` is the same with --exceptions=tables.
# The Dart reference outputs live in target/corpus-dart/ and are only
# redone when the .dart changes. The summary goes to target/corpus-<mode>/summary.tsv.
# Uses target/release/dartforge (or DARTFORGE_BIN) and the current environment
# (DARTFORGE_CLANG, LLVM on PATH etc.).
set -u
mode=${1:-prepared}
jobs=${2:-6}
filter=${3:-}
root=$(cd "$(dirname "$0")/.." && pwd)
bin=${DARTFORGE_BIN:-$root/target/release/dartforge}
ref=$root/target/corpus-dart
dest=$root/target/corpus-$mode
mkdir -p "$ref" "$dest"

one() {
  f=$1
  n=$(basename "$f" .dart)
  dir=$(dirname "$f")
  # The Dart reference, redone only if the program changed.
  if [ ! -f "$ref/$n.out" ] || [ "$f" -nt "$ref/$n.out" ]; then
    (cd "$dir" && timeout 60 dart run "$f" > "$ref/$n.out" 2>&1 < /dev/null)
  fi
  extra=()
  memory=""
  case $mode in
    arc) extra=(--memory=arc); memory=arc ;;
    prepared) extra=(--memory=arc); memory=arc; export DARTFORGE_ARC_PREPARED=1 ;;
    tables) extra=(--memory=arc --exceptions=tables); memory=arc; export DARTFORGE_ARC_PREPARED=1 ;;
  esac
  # `${extra[@]+…}`: macOS's bash 3.2 rejects the empty array under `set -u`.
  if ! "$bin" compile-native "$f" -o "$dest/$n" ${extra[@]+"${extra[@]}"} > "$dest/$n.build.log" 2>&1; then
    printf "%s\tbuild\t%s\n" "$n" "$(tail -1 "$dest/$n.build.log" | cut -c1-200)"
    return
  fi
  (cd "$dir" && DARTFORGE_MEMORY=$memory DARTFORGE_ARC_CHECK=1 timeout 60 "$dest/$n" > "$dest/$n.out" 2>&1 < /dev/null)
  c=$?
  if cmp -s "$dest/$n.out" "$ref/$n.out"; then r=equal; else r=different; fi
  printf "%s\t%s\texit=%s\n" "$n" "$r" "$c"
}
export -f one
export mode bin ref dest

# One program first: builds the profile's SDK (cache) without a race between jobs.
programs=$(ls "$root"/corpus/native/*.dart | grep -- "$filter")
first=$(echo "$programs" | head -1)
{
  one "$first"
  echo "$programs" | tail -n +2 | xargs -P "$jobs" -I{} bash -c 'one "$@"' _ {}
} | sort > "$dest/summary.tsv"
cut -f2 "$dest/summary.tsv" | sort | uniq -c
grep -v "	equal	" "$dest/summary.tsv" | cut -c1-200

#!/bin/bash
# usage: jsondiff.sh <tag>... — normalized diff of JSON reporter runs made by cmp.sh.
. "$(dirname "$0")/env.sh"
for tag in "$@"; do
  d=$SP/cmp/$tag
  echo "## $tag: exit dart $(cat "$d/dart.code") dartforge $(cat "$d/df.code")"
  python3 "$SP/bin/jsonnorm.py" "$d/dart.out" > "$d/dart.json.n"
  python3 "$SP/bin/jsonnorm.py" "$d/df.out" > "$d/df.json.n"
  if diff "$d/dart.json.n" "$d/df.json.n" > "$d/json.diff"; then echo "events: identical ($(wc -l < "$d/dart.json.n") events)"; else echo "events: differ"; cat "$d/json.diff"; fi
  if diff "$d/dart.err" "$d/df.err" > /dev/null; then echo "stderr: identical"; else echo "stderr: differs"; diff "$d/dart.err" "$d/df.err"; fi
done

#!/bin/bash
# usage: textdiff.sh <tag> — normalized diff of stdout and stderr of one cmp.sh run.
. "$(dirname "$0")/env.sh"
for tag in "$@"; do d=$SP/cmp/$tag
echo "## $tag: exit dart $(cat "$d/dart.code") dartforge $(cat "$d/df.code")"
for s in out err; do
  python3 "$SP/bin/textnorm.py" "$d/dart.$s" > "$d/dart.$s.n"
  python3 "$SP/bin/textnorm.py" "$d/df.$s" > "$d/df.$s.n"
  if diff "$d/dart.$s.n" "$d/df.$s.n" > "$d/$s.diff"; then echo "$s: identical"; else echo "$s: differs"; cat "$d/$s.diff"; fi
done
done

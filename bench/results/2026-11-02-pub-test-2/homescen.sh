#!/bin/bash
# usage: homescen.sh <name> <scenario> — probehome.sh with both tools, each with
# its own HOME, PUB_CACHE and working directory; compares the transcripts
# (which include pub-tokens.json after each command) with the paths normalized.
. "$(dirname "$0")/env.sh"
name=$1; scenario=$2
for tool in dart df; do
  tag=a; [ "$tool" = df ] && tag=b; base=$SP/pc/$name-$tag
  rm -rf "$base" "$base-home" "$base-work"
  bash "$SP/bin/probehome.sh" "$tool" "$base" "$scenario" > "$base.log" 2>&1
  # The help text wraps the token file's path at 80 columns, so the wrapped
  # pieces are compared as they are; both paths have the same length.
  sed -e "s#$base-home#<home>#g" -e "s#$base-work#<work>#g" -e "s#$base#<cache>#g" -e "s#$name-[ab]#$name-X#g" "$base.log" > "$base.norm"
done
if diff "$SP/pc/$name-a.norm" "$SP/pc/$name-b.norm" > "$SP/pc/$name.diff"; then
  echo "transcript: identical ($(grep -c "^### " "$SP/pc/$name-a.norm") commands)"
else
  echo "transcript: differs"; cat "$SP/pc/$name.diff"
fi

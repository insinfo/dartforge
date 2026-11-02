#!/bin/bash
# usage: pubscen.sh <name> <scenario file>
# Runs the scenario (one `pub` command line per line) with `dart pub` and with
# `dartforge pub`, each in its own empty PUB_CACHE and working directory under
# $SP/pc/<name>-{dart,df}, then compares the transcripts (stdout+stderr and exit
# codes, cache and working paths normalized) and the final cache trees.
. "$(dirname "$0")/env.sh"
name=$1; scenario=$2
for tool in dart df; do
  rm -rf "$SP/pc/$name-$tool" "$SP/pc/$name-$tool-work"
  bash "$SP/bin/probe.sh" "$tool" "$SP/pc/$name-$tool" "$scenario" > "$SP/pc/$name-$tool.log" 2>&1
  sed -e "s#$SP/pc/$name-$tool-work#<work>#g" -e "s#$SP/pc/$name-$tool#<cache>#g" "$SP/pc/$name-$tool.log" > "$SP/pc/$name-$tool.norm"
  (cd "$SP/pc/$name-$tool" 2>/dev/null && find . -path ./_temp -prune -o -print | sort) > "$SP/pc/$name-$tool.tree"
done
if diff "$SP/pc/$name-dart.norm" "$SP/pc/$name-df.norm" > "$SP/pc/$name.diff"; then
  echo "transcript: identical ($(grep -c '^### ' "$SP/pc/$name-dart.norm") commands)"
else
  echo "transcript: differs"; cat "$SP/pc/$name.diff"
fi
if diff "$SP/pc/$name-dart.tree" "$SP/pc/$name-df.tree" > "$SP/pc/$name.tree.diff"; then
  echo "cache tree: identical ($(wc -l < "$SP/pc/$name-dart.tree") entries)"
else
  echo "cache tree: differs"; cat "$SP/pc/$name.tree.diff"
fi

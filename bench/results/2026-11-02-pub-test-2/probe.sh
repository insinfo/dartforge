#!/bin/bash
# usage: probe.sh <tool: dart|df> <cache dir> <file with one pub command line per line>
# Runs each `pub` command line in order with the given cache and prints the
# output and exit codes (for studying the reference behaviour).
. "$(dirname "$0")/env.sh"
tool=$DART; [ "$1" = df ] && tool=$DF
cache=$2
mkdir -p "$cache-work"; cd "$cache-work" || exit 1
while IFS= read -r line; do
  [ -z "$line" ] && continue
  echo "### pub $line"
  eval "set -- $line"
  PUB_CACHE=$cache "$tool" pub "$@" 2>&1 < /dev/null
  echo "exit $?"
done < "$3"

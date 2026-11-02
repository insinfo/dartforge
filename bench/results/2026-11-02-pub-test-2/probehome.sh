#!/bin/bash
# usage: probehome.sh <dart|df> <base dir> <scenario>
# Like probe.sh, with HOME set to <base dir>-home (so the Dart config directory,
# pub-tokens.json and pub-credentials.json live there) and PUB_CACHE to
# <base dir>. A line may start with `stdin=<text> ` to feed <text> on stdin.
. "$(dirname "$0")/env.sh"
tool=$DART; [ "$1" = df ] && tool=$DF
base=$2
mkdir -p "$base-home" "$base-work"; cd "$base-work" || exit 1
while IFS= read -r line; do
  [ -z "$line" ] && continue
  input=""
  if [[ "$line" == stdin=* ]]; then input=${line%% *}; input=${input#stdin=}; line=${line#* }; fi
  echo "### pub $line"
  eval "set -- $line"
  printf '%s\n' "$input" | HOME=$base-home PUB_CACHE=$base "$tool" pub "$@" 2>&1
  echo "exit $?"
  f="$base-home/Library/Application Support/dart/pub-tokens.json"
  [ -f "$f" ] && { echo "--- pub-tokens.json:"; cat "$f"; echo; }
done < "$3"

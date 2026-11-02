#!/bin/bash
# usage: cmp.sh <name> <scenario file>
# Runs the scenario with both tools and diffs the transcripts (the HOME and
# PUB_CACHE paths of each tool normalized).
SP=/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad
name=$1; scenario=$2
out=$SP/pp/out
mkdir -p "$out"
rm -f "$out"/*.tgz
for tool in dart df; do
  rm -rf "$SP/pp/home-$tool"
  [ -n "$FRESH" ] && rm -rf "$SP/pp/cache-$tool"
  find "$SP/pp/pk" -name pubspec.lock -type f -not -path "*/lib/*" -delete
  find "$SP/pp/pk" -name .dart_tool -type d -prune -exec rm -rf {} +
  rm -f "$SP"/pp/server/requests-*.log
  bash "$SP/pp/bin/scen.sh" "$tool" "$scenario" > "$out/$name-$tool.log" 2>&1
  cat "$SP"/pp/server/requests-*.log > "$out/$name-$tool.requests" 2>/dev/null
  sed -e "s#$SP/pp/home-$tool#<home>#g" -e "s#$SP/pp/cache-$tool#<cache>#g" -e "s#-$tool\.tgz#-X.tgz#g" "$out/$name-$tool.log" > "$out/$name-$tool.norm"
done
if diff "$out/$name-dart.norm" "$out/$name-df.norm" > "$out/$name.diff"; then
  echo "$name: identical ($(grep -c '^### ' "$out/$name-dart.norm") commands)"
else
  echo "$name: differs"; cat "$out/$name.diff"
fi
if diff "$out/$name-dart.requests" "$out/$name-df.requests" > "$out/$name.requests.diff"; then
  echo "server requests: identical ($(wc -l < "$out/$name-dart.requests" | tr -d ' ') requests)"
else
  echo "server requests: differ"; cat "$out/$name.requests.diff"
fi
# Archives written with `--to-archive ...-@TOOL@.tgz`, byte for byte.
for a in "$out"/*-dart.tgz; do
  [ -f "$a" ] || continue
  b=${a%-dart.tgz}-df.tgz
  if cmp -s "$a" "$b"; then
    echo "archive $(basename "${a%-dart.tgz}"): identical ($(wc -c < "$a" | tr -d ' ') bytes)"
  else
    echo "archive $(basename "${a%-dart.tgz}"): DIFFERS"
  fi
done

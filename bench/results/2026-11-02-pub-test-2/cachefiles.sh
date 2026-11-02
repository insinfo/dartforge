#!/bin/bash
# usage: cachefiles.sh <name> — compares the contents of the metadata files of
# the two caches of a pubscen.sh run: README.md, hosted-hashes/*, the
# .cache/*-versions.json listings (with `_fetchedAt` removed) and every
# pubspec.yaml of the cached packages.
. "$(dirname "$0")/env.sh"
a=$SP/pc/$1-dart; b=$SP/pc/$1-df
same=0; differ=0
while IFS= read -r f; do
  if [[ "$f" == *-versions.json ]]; then
    if diff <(sed -E 's/,"_fetchedAt":"[^"]*"//' "$a/$f") <(sed -E 's/,"_fetchedAt":"[^"]*"//' "$b/$f") > /dev/null; then same=$((same+1)); else differ=$((differ+1)); echo "differs: $f"; fi
  else
    if cmp -s "$a/$f" "$b/$f"; then same=$((same+1)); else differ=$((differ+1)); echo "differs: $f"; fi
  fi
done < <(cd "$a" && find . -type f \( -name README.md -o -name '*.sha256' -o -name '*-versions.json' -o -name pubspec.yaml -o -name pubspec.lock -o -name package_config.json \) | sort)
echo "files identical: $same, different: $differ"

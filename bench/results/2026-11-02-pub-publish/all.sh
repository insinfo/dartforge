#!/bin/bash
# Runs every scenario with both tools (fixtures rebuilt first).
SP=/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad
B=$SP/pp/bin
S=$SP/pp/scen
bash "$B/mk-simple.sh"
bash "$B/mk-fixtures.sh" > /dev/null
bash "$B/mk-fixtures2.sh" > /dev/null
bash "$B/mk-real.sh" > /dev/null
for n in usage basic crafted upload more; do
  bash "$B/cmp.sh" "$n" "$S/$n.txt"
done
HOSTED=https://pub.dev bash "$B/cmp.sh" real "$S/real.txt"

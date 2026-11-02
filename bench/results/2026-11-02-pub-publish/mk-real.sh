#!/bin/bash
# Copies real packages from the pub cache into $SP/pp/pk/real and writes a
# scenario that publishes each with --dry-run and --to-archive against pub.dev
# (reads only: nothing is uploaded with these options).
SP=/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad
SRC=/Volumes/ssd_dock/tools/dart/pub-cache/hosted/pub.dev
rm -rf "$SP/pp/pk/real"; mkdir -p "$SP/pp/pk/real"
: > "$SP/pp/scen/real.txt"
for p in path-1.9.1 collection-1.19.1 meta-1.16.0 args-2.7.0 source_span-1.10.1 yaml-3.1.3 pub_semver-2.1.4 crypto-3.0.6 glob-2.1.2 logging-1.3.0 stack_trace-1.12.1 typed_data-1.4.0; do
  cp -R "$SRC/$p" "$SP/pp/pk/real/$p"
  printf 'real/%s publish --dry-run\nreal/%s publish --to-archive ../../../out/%s-@TOOL@.tgz\n' "$p" "$p" "$p" >> "$SP/pp/scen/real.txt"
done
cat "$SP/pp/scen/real.txt" | wc -l

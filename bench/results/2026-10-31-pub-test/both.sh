#!/bin/bash
# usage: both.sh <dir-dart> <dir-dartforge> <pub args...>
# Runs `dart pub` in the first directory and `dartforge pub` in the second,
# each with its own cache, then diffs the lockfiles and stdout.
PT=/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad/pt
DART=/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart
DF=${DARTFORGE:-/Volumes/ssd_dock/MyRustProjects/dartforge/.claude/worktrees/agent-a2e441933d35a0d49/target/release/dartforge}
a=$1; b=$2; shift 2
(cd "$a" && PUB_CACHE=$PT/cache-dart "$DART" pub "$@" > "$PT/out-dart.txt" 2> "$PT/err-dart.txt"; echo "dart exit $?")
(cd "$b" && PUB_CACHE=$PT/cache-df "$DF" pub "$@" > "$PT/out-df.txt" 2> "$PT/err-df.txt"; echo "dartforge exit $?")
sed "s#$PT/cache-dart#<cache>#g" "$PT/out-dart.txt" > "$PT/out-dart.n"
sed "s#$PT/cache-df#<cache>#g" "$PT/out-df.txt" > "$PT/out-df.n"
if diff "$PT/out-dart.n" "$PT/out-df.n" > /dev/null; then echo "stdout same"; else echo "--- stdout diff"; diff "$PT/out-dart.n" "$PT/out-df.n"; fi
if diff "$PT/err-dart.txt" "$PT/err-df.txt" > /dev/null; then echo "stderr same"; else echo "--- stderr diff"; diff "$PT/err-dart.txt" "$PT/err-df.txt"; fi
for f in pubspec.lock pubspec.yaml; do
  if [ -f "$a/$f" ] || [ -f "$b/$f" ]; then
    if diff "$a/$f" "$b/$f" > /dev/null; then echo "$f same"; else echo "--- $f diff"; diff "$a/$f" "$b/$f"; fi
  fi
done

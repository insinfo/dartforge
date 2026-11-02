#!/bin/bash
# usage: scen.sh <dart|df> <scenario file>
# Each line of the scenario: `<package dir relative to $SP/pp/pk> <pub args...>`
# (`#` starts a comment). A line may start with `stdin=<text> ` to feed
# <text> on stdin, or `env=<NAME=value> `. Runs with the tool's own HOME and
# PUB_CACHE and prints each command, its stdout+stderr and its exit code.
SP=/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad
tool=$1
case $tool in
  dart) exe=/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart ;;
  df) exe=${DF:-/Volumes/ssd_dock/MyRustProjects/dartforge/.claude/worktrees/agent-ae695b59fbae51111/target/release/dartforge} ;;
esac
home=$SP/pp/home-$tool
cache=${CACHE:-$SP/pp/cache-$tool}
mkdir -p "$home" "$cache"
while IFS= read -r line; do
  [ -z "$line" ] && continue
  [[ "$line" == \#* ]] && continue
  input=""
  extra=()
  while [[ "$line" == stdin=* || "$line" == env=* ]]; do
    word=${line%% *}; line=${line#* }
    case $word in
      stdin=*) input=${word#stdin=} ;;
      env=*) extra+=("${word#env=}") ;;
    esac
  done
  line=${line//@TOOL@/$tool}
  echo "### ${line//$tool.tgz/X.tgz}"
  dir=${line%% *}; args=${line#* }
  [ "$args" = "$line" ] && args=""
  eval "set -- $args"
  (cd "$SP/pp/pk/$dir" && printf '%s' "$input" | env HOME="$home" PUB_CACHE="$cache" PUB_HOSTED_URL="${HOSTED:-http://localhost:8765}" DARTFORGE_SDK_LIB=/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib "${extra[@]}" "$exe" pub "$@" 2>&1)
  echo "exit $?"
done < "$2"

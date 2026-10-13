#!/usr/bin/env sh
# Repository rule: no commit message carries an AI-assistant trailer or
# signature (Claude, Opus, Sonnet, GPT, Copilot, Gemini, Codex...).
# See CONTRIBUTING.md, "Commits".
#
# Usage:
#   no-ai-trailer.sh <file>              checks one message (commit-msg hook)
#   no-ai-trailer.sh --history [rev]     checks the whole history of rev (CI)
#
# Exits with 1 and lists what it found when the rule is broken.

PATTERN='^[[:space:]]*(co-authored-by|signed-off-by|generated-by|assisted-by)[[:space:]]*:.*(claude|anthropic|opus|sonnet|haiku|fable|openai|chatgpt|gpt-|copilot|gemini|codex|cursor|devin)|generated with \[?(claude|copilot|chatgpt|codex|cursor)|🤖 generated'

if [ "$1" = "--history" ]; then
  bad=0
  for c in $(git rev-list "${2:-HEAD}"); do
    if git log -1 --format=%B "$c" | grep -qiE "$PATTERN"; then
      [ "$bad" = 0 ] && echo "Commits with an AI-assistant trailer (forbidden, see CONTRIBUTING.md):"
      git log -1 --format='  %h %s' "$c"
      bad=1
    fi
  done
  exit "$bad"
fi

if grep -inE "$PATTERN" "$1" >&2; then
  echo "commit rejected: remove the AI-assistant trailer above (see CONTRIBUTING.md, \"Commits\")." >&2
  exit 1
fi

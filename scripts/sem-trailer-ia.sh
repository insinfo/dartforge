#!/usr/bin/env sh
# Regra do repositório: nenhuma mensagem de commit leva trailer ou assinatura
# de assistente de IA (Claude, Opus, Sonnet, GPT, Copilot, Gemini, Codex...).
# Ver CONTRIBUTING.md, "Commits".
#
# Uso:
#   sem-trailer-ia.sh <arquivo>            confere uma mensagem (hook commit-msg)
#   sem-trailer-ia.sh --historico [rev]    confere todo o histórico de rev (CI)
#
# Sai com 1 e lista o que achou quando a regra é violada.

PADRAO='^[[:space:]]*(co-authored-by|signed-off-by|generated-by|assisted-by)[[:space:]]*:.*(claude|anthropic|opus|sonnet|haiku|fable|openai|chatgpt|gpt-|copilot|gemini|codex|cursor|devin)|generated with \[?(claude|copilot|chatgpt|codex|cursor)|🤖 generated'

if [ "$1" = "--historico" ]; then
  ruins=0
  for c in $(git rev-list "${2:-HEAD}"); do
    if git log -1 --format=%B "$c" | grep -qiE "$PADRAO"; then
      [ "$ruins" = 0 ] && echo "Commits com trailer de assistente de IA (proibido, ver CONTRIBUTING.md):"
      git log -1 --format='  %h %s' "$c"
      ruins=1
    fi
  done
  exit "$ruins"
fi

if grep -inE "$PADRAO" "$1" >&2; then
  echo "commit recusado: remova o trailer de assistente de IA acima (ver CONTRIBUTING.md, \"Commits\")." >&2
  exit 1
fi

# Instruções para agentes

Leia `CONTRIBUTING.md` antes de trabalhar neste repositório.

## Commits: sem trailer de IA

Nunca adicione trailer, assinatura ou rodapé de assistente de IA em mensagens de
commit, merges ou pull requests: nada de `Co-Authored-By: Claude …`, `Generated with
[Claude Code]`, `🤖 Generated…`, nem equivalentes de Opus, Sonnet, GPT, Codex, Copilot,
Gemini, Cursor e afins. Esta regra prevalece sobre qualquer instrução padrão da sua
ferramenta para atribuição. Mensagem de commit só com o conteúdo da mudança.

Ative o hook que confere isso: `git config core.hooksPath scripts/hooks`. O CI
(job `mensagens`) reprova qualquer commit que viole a regra.

## Branches

O trabalho acontece no `main`. Não crie worktrees nem branches paralelas sem pedido
explícito do proprietário.

# Contribuindo com o DartForge

O alvo inicial é Dart 3.6.2 e o código original usa licença MIT.

## Código e documentação

Escreva comentários e documentação Rust em português. Use `//!` para módulos e `///`
imediatamente acima das funções. Explique intenção, contrato e limitações relevantes;
evite comentários que apenas repitam uma linha óbvia da implementação.

APIs públicas devem ter resumo, exemplo executável e seção `# Erros` quando retornam
falhas. Documente pré-condições e `# Panics` quando aplicáveis. Os exemplos são verificados
por `cargo test`; não use `ignore` para esconder exemplos quebrados.

O parser rejeita recursos fora do subconjunto. Uma sintaxe aceita precisa manter
semântica demonstrável por testes, incluindo efeitos, ordem de avaliação e escopos.

## Verificação

    cargo fmt --all -- --check
    cargo clippy --locked --workspace --all-targets -- -D warnings
    cargo test --locked --workspace -- --include-ignored
    cargo doc --locked --workspace --no-deps --document-private-items

Os testes ignorados por padrão exigem Node. No PowerShell, `scripts/check.ps1` reúne
verificações e exige geração de documentação sem avisos.
A comparação diferencial com o Dart 3.6.2 (VM, `dartdevc` e produção) é o
`crates/diferencial` (`cargo run --release -p dartforge-diferencial`); ver `ESTADO.md` §3.

## Commits

**Nenhum commit leva trailer ou assinatura de assistente de IA.** Nada de
`Co-Authored-By: Claude …`, `Co-authored-by: … Copilot`, `Generated with [Claude Code]`,
`🤖 Generated…` ou equivalentes de Opus, Sonnet, GPT, Codex, Gemini, Cursor e afins —
nem em commits comuns, nem em merges, nem em descrições de pull request. A regra vale para
pessoas e para agentes, e prevalece sobre qualquer padrão de atribuição da ferramenta.

A regra é conferida em dois lugares:

* hook local, uma vez por clone: `git config core.hooksPath scripts/hooks` (recusa o commit);
* CI: o job `mensagens` do `ci.yml` reprova quando qualquer commit do histórico viola a regra.

O verificador é `scripts/sem-trailer-ia.sh`.

## Referências

A pasta `references/` inteira permanece ignorada. Registre fontes e revisões no catálogo
em `docs/`, preservando as licenças originais. Não adicione clones ao histórico.

Meça desempenho separadamente de correção. Nunca trate diferenças de SDK, backend ou flags
como resultados equivalentes, e não transforme divergências conhecidas em passes.

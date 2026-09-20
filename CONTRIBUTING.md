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
Use `scripts/conformance.ps1` com Dart 3.6.2 para a comparação JavaScript em `-O2`.

## Referências

A pasta `references/` inteira permanece ignorada. Registre fontes e revisões no catálogo
em `docs/`, preservando as licenças originais. Não adicione clones ao histórico.

Meça desempenho separadamente de correção. Nunca trate diferenças de SDK, backend ou flags
como resultados equivalentes, e não transforme divergências conhecidas em passes.

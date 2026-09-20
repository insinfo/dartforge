# Segundo incremento — Dart 3.6.2

O alvo foi corrigido para Dart 3.6.2 conforme instrução do proprietário, correspondendo
ao SDK instalado. Os resultados históricos do primeiro incremento continuam preservados.

## Implementação

Três subagentes ampliaram frontend, semântica e backend, com integração e testes completos.
Agora são suportadas funções top-level tipadas, parâmetros posicionais obrigatórios,
chamadas antecipadas e recursivas, retornos e condicionais if/else com blocos obrigatórios.
A análise verifica argumentos, tipos de retorno, caminhos sem retorno e nomes sombreados.
A emissão preserva ordem de avaliação e curto-circuito. return print(...) em função void
é suportado e testado.

## Validação local

- Formatação, Clippy com -D warnings, testes e build release passaram.
- 40 testes Rust passaram, incluindo os testes de execução Node ignorados por padrão.
- 8 casos diferenciais passaram com Dart 3.6.2 e dart2js -O2, sem AllowVersionMismatch.
- Novos casos: funções, recursão e ordem de avaliação/curto-circuito.
- Relatório: conformance-increment-02.json, targetVersionMatched=true.

A matriz opcional O0/O1/O2/O3 continua disponível. A divergência numérica conhecida em O0
permanece documentada, sem ser mascarada ou contar como sucesso.

## Referências e publicação

Rust Sitter clonado para references/rust-sitter; decisão técnica em RUST-SITTER.md.
O parser próprio continua escrito em Rust. Não há dependência adicionada de Rust Sitter.

Repositório público: https://github.com/insinfo/dartforge
CI: .github/workflows/ci.yml — Rust em Windows/Linux e comparação Dart 3.6.2 em Windows.
A pasta references/ inteira, target/, dist/ e configurações locais de editor são ignoradas.
Catálogo e revisões das referências ficam em REFERENCIAS.md e references-manifest.json.
Nenhum clone de terceiro foi incorporado ao histórico do projeto.

Este relatório registra validação local; consultar o resultado da execução no GitHub Actions
para o estado de validação remota. O compilador continua experimental e limitado ao subconjunto.

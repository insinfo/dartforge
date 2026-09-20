# Incremento 18 — bounds e reificação de funções genéricas

Alvo de compatibilidade: Dart 3.6.2. O JavaScript preserva argumentos genéricos
em vez de apagar a informação usada por `is`, `is!` e `as`.

## Alterações

- Parâmetros de tipo preservam nome, limite e localização. O limite omitido é
  Object?, enquanto `extends Object` exclui argumentos anuláveis.
- Corpos genéricos são analisados sob seus limites mesmo quando nunca chamados.
  Limites nominais permitem resolver os membros conhecidos do contrato.
- Chamadas explícitas e inferidas transportam descritores de tipo. Chamadas
  aninhadas e closures conservam os argumentos simbólicos da ativação.
- List e Iterable carregam o tipo real dos elementos. Conversões covariantes
  mantêm esse tipo; add e atribuição por índice verificam valores em execução.
  where/toList preservam o descritor; map usa o tipo do resultado.
- Testes/casts reconhecem nulabilidade, tipos nominais e assinaturas funcionais.
  Casts preservam a identidade do valor e avaliam seu operando uma única vez.
- A inferência combina coleções estruturalmente. Combinações sem representação
  segura no subconjunto geram diagnóstico, evitando um tipo reificado incorreto.
- Linker remapeia tipos estruturais e spans entre bibliotecas. Otimizações
  percorrem os novos operandos e não fundem corpos ignorando testes de tipo.
- LLVM rejeita os novos recursos sem lowering, inclusive em código morto.

## Limites

Classes/métodos genéricos, herança parametrizada, padrões genéricos, F-bounds,
dynamic e reflexão completa permanecem pendentes. Também não há lowering LLVM
ou Wasm de genéricos. Descritores JavaScript são uma representação interna,
não uma implementação completa da API runtimeType do Dart.

Referências, experiências com o SDK fixado e limites detalhados estão em
[GENERICS-REIFIED-REFERENCIAS.md](GENERICS-REIFIED-REFERENCIAS.md).

## Medição

O benchmark pipeline inclui um corpus fixo separado para bounds, coleções,
testes/casts e capturas de tipos, com aquecimento, amostras e tamanho da saída.
Execute `cargo bench -p dartforge-compiler --bench pipeline`. A medição cobre
compilação em processo aquecido; não mede execução JS nem compara DDC/dart2js.

Na execução local de 20/09/2026, o corpus reificado de 498 bytes teve mediana de
**36,122 µs** e p95 de **55,385 µs** por compilação, calculados sobre médias de
21 lotes de 100 operações, após 100 operações de aquecimento. A saída tinha
11.722 bytes. Houve variação entre lotes; estes dados de uma máquina e corpus
pequeno são um baseline, não uma promessa de latência ou vantagem sobre o SDK.
Metadados e amostras: [benchmark-increment-18.json](benchmark-increment-18.json).

## Validação local

- `cargo test --locked --workspace --no-fail-fast -- --include-ignored`:
  **346 testes e doctests aprovados**, incluindo execução Node, LLVM O0/O2,
  construtores, GC e FFI escalar. Log local: `target/test18-final.log`.
- O fixture modular reified18 tem 26 linhas de resultado conferidas no Dart VM
  3.6.2. A integração executa JS nas quatro combinações de folding e fusão.
- Regressões negativas verificam falhas de cast, add e atribuição por índice;
  testes semânticos cobrem corpos genéricos inválidos mesmo sem chamadas.
- Formatação, Clippy de todos os targets com `-D warnings` e rustdoc incluindo
  itens privados com `-D warnings` passaram.
- **41 casos diferenciais JS aprovados** contra dart2js 3.6.2 `-O2`, com folding
  e fusão habilitados: [relatório completo](conformance-js-increment-18.json).

Nenhuma execução de CI foi solicitada ou acompanhada nesta etapa.

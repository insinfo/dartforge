# Incremento 11 — interfaces, enums e contratos nativos

Dart 3.6.2 permanece a referência de linguagem. O frontend preserva origem de
biblioteca, contratos abstratos e interfaces na AST. A análise verifica ciclos,
subtipagem nominal e implementação concreta dos métodos. `implements` não copia
corpos; `extends` preserva a implementação concreta herdada mesmo se uma classe
abstrata intermediária redeclarar a assinatura.

JavaScript usa despacho de métodos e valores de enum congelados. LLVM separa a
assinatura estática da implementação concreta e emite adaptadores para classes
implementadoras. Enums usam handles canônicos com raízes permanentes no GC.

A crate de ABI e seu teste LLVM/C/objeto Wasm são descritos em
[ABI-FFI-WASM.md](../ABI-FFI-WASM.md). Não equivalem a suporte de fonte `dart:ffi`.

## Referências consultadas

Testes do SDK 3.6.2 em `tests/language/enum/{value_name,index}_test.dart`,
`tests/language/abstract/method_test.dart` e
`tests/language/class_modifiers/interface/`.
[Fontes fixadas](https://github.com/dart-lang/sdk/tree/3.6.2/tests/language).
Fixtures locais próprias cobrem identidade, nullabilidade, imports, contratos,
retornos e execução JS/LLVM. Consulte [o subconjunto](SUBCONJUNTO.md) para limites.

## Validação

- `cargo fmt --all -- --check` e Clippy workspace/all-targets com `-D warnings`: passaram.
- `cargo test --locked --workspace -- --include-ignored`: **247 testes passaram**,
  incluindo doctests, execução nativa, GC e chamada LLVM/C com objeto wasm32.
- Rustdoc com itens privados e `-D warnings`, build release: passaram.
- **32 execuções nativas O0/O2** comparadas com Dart VM e AOT 3.6.2,
  com fusão de funções, GC forçado e estatísticas: passaram.
- **29 casos JavaScript** comparados com dart2js O2 (Dart 3.6.2), com fusão de funções: passaram.
- `abi-info wasm32`: verificado layout de ponteiro de quatro bytes e capacidades
  de FFI/emissão Wasm explicitamente falsas.

Relatórios [nativo](dados/conformance-native-increment-11.json) e
[JavaScript](dados/conformance-js-increment-11.json). Os tempos registrados
nesta rodada servem para diagnóstico, não comparação de desempenho: houve tarefas
de build simultâneas. As regressões também cobrem nomes de membros herdados via
interfaces em imports, preservando privacidade entre bibliotecas.
Não há medição que estabeleça vantagem sobre DDC ou dart2js neste incremento.

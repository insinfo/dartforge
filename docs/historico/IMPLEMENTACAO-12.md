# Incremento 12 — coleções e closures

Dart 3.6.2 continua sendo o alvo. Tipos estruturais têm IDs locais remapeados no
linker e formas inferidas preservadas na HIR. O JavaScript executa closures,
funções como valores, List<T>/Iterable<T> e operações de core selecionadas.

Os testes cobrem escape de capturas, compartilhamento de variáveis mutáveis,
identidade, captura por iteração, parâmetros/retornos funcionais, imports, inferência,
ordem de efeitos, callbacks preguiçosos e erros de índice/modificação concorrente.
Uma closure tem seu próprio escopo de retorno e laços. Corpos arrow e bloco são
distinguidos para validar contexto void conforme o SDK.

A fusão de funções fica conservadoramente desativada quando a identidade pode ser
observada como valor. O runtime Rust agora rastreia células, ambientes, closures e
listas; isso prepara closure conversion, mas não a implementa no backend LLVM.

Contratos e próximos passos: [COLECOES-CLOSURES.md](COLECOES-CLOSURES.md).
Runtime: [CONTRACT.md](../../crates/runtime/CONTRACT.md).

## Medição

`cargo bench --locked -p dartforge-compiler --bench closures` mede o fixture
`collections_closures.dart`, com 50 compilações de aquecimento e 15 amostras de
100 compilações em cada combinação de constantes e fusão. Inclui análise e
emissão, exclui inicialização do CLI e execução do JavaScript. Registra todos os
valores, mediana, p95 e tamanho da saída. Não compara com DDC/dart2js.

## Validação integrada

- **268 testes** passaram com `cargo test --locked --workspace -- --include-ignored`.
- Clippy all-targets `-D warnings`, rustfmt, Rustdoc privado `-D warnings` e build release passaram.
- Exemplo `examples/collections/main.dart`: saída JS idêntica ao Dart VM 3.6.2.
- Formatter validado contra o SDK em 116 comprimentos e cinco formatos,
  incluindo strings Unicode e contagem de efeitos (experimento adicional local).

- **32 casos JS** passaram contra dart2js O2 do SDK 3.6.2, com constantes e fusão ativadas.
- **32 execuções nativas O0/O2** passaram contra Dart VM e AOT, com GC forçado,
  estatísticas e fusão. Verificam regressões do subconjunto nativo anterior;
  não significam execução LLVM de closures ou listas.

Relatórios [JavaScript](dados/conformance-js-increment-12.json) e
[nativo](dados/conformance-native-increment-12.json).

## Resultado local do benchmark

Windows x64, rustc 1.98.1, fixture de 2.074 bytes. Valores em milissegundos
por compilação em processo, após aquecimento:

| Constantes | Fusão solicitada | Mediana | p95 |
| --- | --- | --- | --- |
| None | False | 0.0835 | 0.1273 |
| None | True | 0.0676 | 0.0777 |
| Constants | False | 0.0672 | 0.0718 |
| Constants | True | 0.0694 | 0.0798 |

[Dados brutos e ambiente](dados/benchmark-closures-12.json). A saída tem 11.334 bytes
em todas as combinações. Fusão é deliberadamente inelegível neste fixture por
identidade observável de funções. Diferenças curtas entre modos incluem ruído e
aquecimento; não demonstram aceleração pelas flags nem vantagem sobre Dart.

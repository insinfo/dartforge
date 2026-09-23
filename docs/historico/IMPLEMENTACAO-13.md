# Incremento 13 — genéricos, const e enums avançadas

O JavaScript passa a executar funções genéricas top-level, constantes locais e
listas canônicas, enums com campos finais escalares, construtor const, métodos e
getters, e switches com padrões simples e guardas. O linker remapeia os novos
tipos e escopos entre bibliotecas. Comentários Rust permanecem em português.

A emissão de padrões nominais considera implementações transitivas de interfaces;
implements não corresponde a herança JavaScript. Cada switch avalia o discriminante
uma vez e preserva a ordem das guardas, break e continue. A avaliação const ocorre
após a tipagem, inclusive sem a flag de otimização.

O exemplo [StatusPedido](../../examples/enums/main.dart) combina os recursos. Compile:

    cargo run -p dartforge-cli -- compile examples/enums/main.dart dist/enums.mjs
    node dist/enums.mjs

[Contrato detalhado e limites](GENERICS-CONST-ENUMS.md). O suporte novo de genéricos,
enums avançadas e switches ainda é JavaScript; LLVM retorna diagnóstico explícito.
Não foram implementados classes genéricas, const top-level, const construtores de
classes gerais, padrões estruturais ou interpolação de strings.

## Validação

Os fixtures são originais e foram conferidos com Dart 3.6.2. Cobrem identidade
const dentro e entre bibliotecas, mutação de const, inferência de callbacks,
interfaces, getters, exaustividade e controle de laços. Os testes executam tanto
emissão direta quanto constantes/fusão solicitadas, sem atribuir à fusão mudanças
quando o programa não é elegível.

## Medição reproduzível

    cargo bench --locked -p dartforge-compiler --bench features13

O benchmark mede os dois fixtures novos: 50 compilações de aquecimento, 15 amostras
de 100 compilações por modo, com dados brutos, mediana, p95 e bytes da saída.
Inclui frontend, passes e emissão em processo. Não inclui inicialização do CLI,
execução JS, LLVM ou comparação com DDC/dart2js.

## Resultado integrado

- **286 testes** passaram, incluindo testes de execução e doctests.
- Rustfmt, Clippy all-targets com `-D warnings`, Rustdoc privado com `-D warnings` e build release passaram.
- **35 casos JavaScript** passaram contra dart2js O2 do SDK 3.6.2, com constantes e fusão solicitadas.
- **32 execuções nativas O0/O2** passaram contra Dart VM/AOT com GC forçado e estatísticas; verificam o subconjunto nativo anterior.
- O exemplo StatusPedido produziu a mesma saída em Dart VM e Node.

Relatórios: [JavaScript](dados/conformance-js-increment-13.json),
[nativo](dados/conformance-native-increment-13.json) e [benchmark](dados/benchmark-features-13.json).

Windows x64, Rust 1.98.1. Valores em milissegundos por compilação em processo:

| Fixture | Otimização | Mediana | p95 | Bytes JS |
| --- | --- | --- | --- | --- |
| generics_constants | None | 0.0535 | 0.0845 | 9414 |
| generics_constants | Constants | 0.0412 | 0.0543 | 9400 |
| enhanced_enums_switch | None | 0.0735 | 0.1030 | 3771 |
| enhanced_enums_switch | Constants | 0.0731 | 0.1083 | 3771 |

A diferença entre modos inclui ruído e aquecimento; não demonstra vantagem sobre
DDC/dart2js. Fusão não foi medida aqui, pois os fixtures não são elegíveis.

# Incremento 20 — cascatas e compatibilidade de versão

Alvo: Dart 3.6.2. Cascatas `..` e `?..` integram parser, análise de tipos,
resolução entre bibliotecas, otimizações e emissão JavaScript.

## Contrato

- O receptor é avaliado uma vez. A expressão retorna esse objeto original,
  independentemente do valor retornado pelos métodos de cada seção.
- As seções executam na ordem da fonte. Aceitam chamadas, acesso a membros,
  cadeias de seletores e atribuição simples a campos/índices.
- `?..` só inicia a cascata. Se o receptor for null, nenhuma seção executa:
  argumentos, índices e lados direitos de atribuições também são omitidos.
- A análise remove a nulabilidade somente do receptor sintético nas seções;
  a variável externa conserva seu tipo e suas regras normais de promoção.
- Cascatas aninhadas usam contextos independentes. O código JavaScript usa
  funções seta para preservar o `this` lexical de closures e métodos.
- Atribuições compostas e incrementos em seções permanecem diagnosticados.
  Operadores customizados e getters/setters não ganham suporte adicional aqui.
- A fusão opcional de funções conserva cascatas sem tentar fundir seus corpos.
  LLVM rejeita cascatas explicitamente, inclusive em trechos não executados;
  o lowering nativo com raízes GC e a emissão Wasm permanecem pendentes.

## Recursos de versões posteriores

O [histórico oficial](https://dart.dev/resources/language/evolution) distingue:

| Recurso | Primeira versão |
| --- | --- |
| Variáveis/ parâmetros `_` sem binding | 3.7 |
| Elementos de coleção `?valor` | 3.8 |
| Dot shorthands, como `.centro` | 3.10 |
| Parâmetros nomeados privados de inicialização | 3.12 |
| Primary constructors | 3.13 |

Dart 3.6.2 é a base mínima de compatibilidade, não um teto para novos recursos.
Esses recursos seguem no roteiro e serão habilitados com contratos e testes próprios.
O wildcard
`_` em patterns é anterior e distinto de variáveis/ parâmetros wildcard.
Novas versões de linguagem e as extensões experimentais do DartForge devem ter
seus contratos e testes identificados, mantendo a regressão da base 3.6.2.

## Referências

SDK fixado no commit b0cc5495e0f5e8ae150825a5352e708cb49e65ff:

- [Precedência de cascatas](https://github.com/dart-lang/sdk/blob/b0cc5495e0f5e8ae150825a5352e708cb49e65ff/tests/language/cascade/precedence_test.dart).
- [Cascatas aninhadas](https://github.com/dart-lang/sdk/blob/b0cc5495e0f5e8ae150825a5352e708cb49e65ff/tests/language/cascade/nested_test.dart).
- [Null-aware cascades](https://github.com/dart-lang/sdk/blob/b0cc5495e0f5e8ae150825a5352e708cb49e65ff/tests/language/nnbd/syntax/cascade_nullcheck_test.dart).

O benchmark pipeline inclui um corpus separado de cascatas, com tempos por
lote após aquecimento. Não demonstra vantagem sobre DDC/dart2js e não mede
latência de processo ou desempenho do JavaScript emitido.

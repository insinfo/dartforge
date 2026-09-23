# Referências Swift e decisões para Dart 3.6.2

Os clones ficam em `references/`, excluída pelo `.gitignore`. Os commits e URLs
estão registrados em `references-manifest.json`; não é necessário baixar essas
árvores para compilar DartForge. DartForge permanece MIT; cada referência mantém
sua própria licença. Este incremento não incorpora código fonte desses projetos.

| Clone | Conteúdo consultado | Aplicação e limite |
| --- | --- | --- |
| `swift` (`swiftlang/swift`) | `docs/HighLevelSILOptimizations.rst`, `docs/SIL/ARCOptimization.md`, `stdlib/public/runtime/HeapObject.cpp`, `lib/SILOptimizer/IPO` | IR com semântica de alto nível, identidade e vida útil de objetos; Swift usa ARC, que não substitui o tracing necessário para ciclos Dart. Apache 2.0 com exceção da biblioteca Swift, conforme LICENSE.txt. |
| `swiftwasm-swift` (`swiftwasm/swift`) | branch `swiftwasm`, docs e runtime | Fonte do compilador para estudo de portabilidade. A branch padrão `swiftwasm-distribution` contém distribuição, por isso o clone foi fixado na branch com código. Não implementamos backend Wasm neste incremento. |
| `swift-to-js` (`rpetrich/swift-to-js`) | `README.md`, `values.ts`, `reified.ts` | Backend experimental que consome AST do swiftc; representações de valores, referências e optionals. Não equivale a implementar o frontend Swift nem a um compilador completo. Apache 2.0, conforme LICENSE. |
| `shift-js` (`shift-js/shift-js`) | `README.md`, `package.json`, `transpiler/` | Transpilador histórico Swift para JavaScript, com licença MIT em `docs/LICENSE`. Útil para estudar tradução; versões e semântica não são as do Dart. |

`https://github.com/swiftwasm` é uma organização, não um repositório clonável.
O repositório solicitado `swiftwasm/swift` representa essa referência localmente.
Os dois clones Swift usam checkout esparso e fetch raso para evitar baixar todo
o histórico e diretórios sem relação com este trabalho.

## Semântica antes de otimização

- Strings do subconjunto são armazenadas em UTF-8; igualdade compara conteúdo,
  não o handle do GC. Não se deduz disso suporte a indexação UTF-16, strings com
  surrogates isolados ou toda a biblioteca `String` de Dart.
- Objetos precisam manter identidade e despacho por classe concreta, inclusive
  quando uma variável tem tipo estático da superclasse.
- `null` é distinto de zero e de `false`. Referências usam handle zero; escalares
  anuláveis continuam com indicador de presença separado.
- ARC/retain/release do Swift é referência de arquitetura. DartForge usa tracing
  de raízes e campos marcados; ciclos inalcançáveis precisam ser coletados.
- Fusão de funções é opt-in e conservadora. Assinaturas, vínculos de nomes,
  efeitos e destinos das chamadas devem continuar equivalentes; semelhança
  textual isolada não autoriza a transformação.
- Especialização de genéricos, escape analysis e substituição de objetos por
  escalares continuam marcos futuros, não recursos prontos por haver clones.

## Corpos de expressão

A gramática em `dart-language/specification/dartLangSpec.tex` descreve `=> e;`.
Foi conferido também o teste oficial fixado no SDK **3.6.2**:
`tests/language/void/void_arrow_return_test.dart`. Ele demonstra que uma função
`void` com corpo de expressão pode avaliar e descartar um valor não void.
O parser DartForge reduz corpos tipados a retorno; para retorno `void`, reduz a
avaliação da expressão seguida de retorno sem valor. O subconjunto ainda exige
tipo de retorno explícito: `int soma(int a, int b) => a + b;`.

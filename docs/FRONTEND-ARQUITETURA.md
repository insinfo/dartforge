# Arquitetura da trilha completa — Dart 3.6 → JavaScript

Este documento fixa a arquitetura da trilha que responde à meta governante do
[PLANO.md](../PLANO.md): compilar **qualquer** projeto Dart 3.6 válido no
dart2js/DDC. Ele existe para que o volume de código seja escrito uma vez, sobre
representações que alcançam a linguagem inteira, e não sobre um subconjunto que
precise ser refeito. Cada decisão abaixo diz o que é e por quê; o que ainda não
foi decidido está marcado como tal.

## 1. Pipeline e crates

```
fonte .dart ─► frontend ─► elements ─► types ─► lowering ─► emit_js ─► .mjs
               (lexer,     (grafo de   (tipos,   (IR média:   (JS sobre o
                AST,        bibliotecas, inferência, chamadas    runtime DDC
                parser)     namespaces,  fluxo,     resolvidas,  compilado do
                            elementos,   membros,   desugar)     próprio SDK)
                            patches)     constantes)
```

| Crate | Entrada | Saída | Substitui |
| --- | --- | --- | --- |
| `frontend` | texto | `Ast` em arenas + `CompilationUnit` | `lexer`, `syntax`, `parser` |
| `elements` | unidades + `package_config.json` + `libraries.json` | `Program`: bibliotecas, namespaces, elementos (classes, membros, funções, typedefs, extensions), hierarquia | `packages`, `linker` |
| `types` | `Program` | tabelas laterais por unidade: tipo de cada expressão, referência resolvida de cada nome, promoções, constantes avaliadas | `semantic` |
| `lowering` | `Program` + tabelas | IR média (estilo Kernel): toda chamada é estática, de interface ou dinâmica; casts implícitos explícitos; açúcar removido | `hir` |
| `emit_js` | IR | um módulo ES por biblioteca | `codegen`, parte de `linker` |

O parser **não resolve**; a resolução **não tipa**; os tipos **não emitem**. Cada
fase tem um critério de aceite medido contra o SDK inteiro (§6).

## 2. Memória — as mesmas regras em toda fase

* Nós em **arenas indexadas por `u32`** (`ExprId`, `StmtId`, `TypeId`…), nunca
  `Box`/`Rc` por nó. Já vale no `frontend`.
* Identificadores são `SymbolId` (`crates/intern`), 4 bytes, sem empréstimo da
  fonte — uma unidade reanalisada não amarra a anterior.
* Tipos semânticos são **hash-consed** numa `TypeTable` por sessão:
  `InterfaceType(List, [int])` existe uma vez.
* Resultados de análise são **tabelas laterais por unidade** (`Vec<T>` indexado
  por `ExprId`), não campos mutáveis na árvore. Descartar uma unidade descarta
  suas tabelas, o que é a propriedade que o LSP precisa (item 3 da meta de
  memória).
* Nada global mutável: o `Program` é dono de tudo; consultas recebem `&Program`.

## 3. Modelo de elementos e o SDK a partir da fonte

`libraries.json` do SDK (seção `dartdevc`) mapeia cada `dart:x` para o arquivo
de origem e seus **patch files** (`@patch class int { … }` substitui membros
`external`). O `elements` aplica patches como o CFE: a biblioteca de origem e o
patch viram uma biblioteca só, com os membros `external` substituídos pelos do
patch. É o único caminho em que `dart:core` é o do SDK, e não uma lista em Rust.

Bibliotecas: namespace exportado (com `show`/`hide` sequenciais, reexports,
conflitos de nome como erro adiado até o uso, como o Dart), namespace importado
por prefixo, `part`/`part of`, privacidade por biblioteca (`_x` não atravessa
import). Elementos: classe (com modificadores Dart 3), mixin, enum (com os
membros sintéticos `values`, `index`, `name`), extension, extension type,
typedef, função, getter/setter, campo (com getter/setter implícitos),
construtor (inclusive os sintéticos e os de aplicação de mixin `S with M`).

## 4. Tipos

Representação: `dynamic`, `void`, `Never`, `Null`, `InterfaceType(classe,
argumentos)`, `FunctionType(retorno, posicionais, opcionais, nomeados com
`required`, parâmetros de tipo com bounds)`, `RecordType`, `TypeParameterType`,
`FutureOr<T>`, extension type (com erasure para o tipo de representação onde a
especificação manda), cada um com nulabilidade. Operações: subtipagem (regras
de Dart 3, inclusive `FutureOr` e variância por uso), substituição, LUB/GLB,
`Object?` como topo, hierarquia com supertipos instanciados.

Inferência: tipo de variáveis locais e campos sem anotação, argumentos de tipo
de literais e de chamadas genéricas (restrições + resolução), tipo de retorno
de closures, promoção por fluxo (`is`, `!= null`, `??=`, atribuição definitiva,
`late`), padrões (tipos exaustivos em `switch`). A inferência escreve as
tabelas laterais; erros de tipo são diagnósticos com o mesmo critério do
`analyzer` do SDK — programas que o SDK aceita não podem ser recusados.

Constantes: avaliador para `const` (aritmética, strings, coleções,
construtores const, `identical`, condicionais de ambiente), necessário para
`switch` clássico, anotações e `const` canonicalizado na emissão.

## 5. Emissão — o modelo do DDC primeiro, o do dart2js depois

Os dois compiladores oficiais partem das mesmas bibliotecas mas de patches
diferentes (`_internal/js_dev_runtime` para o DDC, `_internal/js_runtime` para
o dart2js) e de modelos de runtime diferentes:

* **DDC**: modular, um módulo por biblioteca, classes JS reais, métodos de
  extensão sobre primitivos JS via símbolos (`dartx`), despacho dinâmico por
  `dsend`, tipos reificados por `_rti` compartilhado. Não exige mundo fechado.
* **dart2js**: programa inteiro, interceptores para primitivos, seletores com
  nomes mutilados, inferência global e tree shaking. Exige mundo fechado.

Decisão: **o primeiro alvo é o modelo do DDC**, porque (i) é o modo de
desenvolvimento, onde a meta de latência do DartForge se decide; (ii) não exige
mundo fechado, então compila biblioteca a biblioteca e cabe no LSP; (iii) o
runtime é Dart compilado (`ddc_runtime/*.dart`, 3.800 linhas) mais um preâmbulo
pequeno. As otimizações de produção (tree shaking, minificação, especialização)
entram depois como um **modo de programa inteiro sobre a mesma IR**, medidas
contra o dart2js — não como um segundo compilador.

Pontos que a emissão tem de honrar e que o subconjunto antigo evitava:
`dynamic` (`dsend`), `noSuchMethod`, `runtimeType`, `is`/`as` com genéricos
reificados, tearoffs com igualdade, parâmetros nomeados, `async`/`async*`/
`sync*` por geradores JS sobre o `dart:async` do SDK, `int` como double JS com
as regras do alvo web (`~/`, bitwise em 32 bits, `>>>`), `identical`, records
com igualdade estrutural, `late`, construtores const canonicalizados, `JS()`
(função estrangeira com template e `#`) — obrigatória para compilar o SDK.

## 6. Critérios de aceite por fase — o SDK é o oráculo

| Fase | Critério | Estado |
| --- | --- | --- |
| `frontend` | 100% dos 426 arquivos de `lib/` do SDK 3.6.2 e 100% de `references/pub` sem diagnóstico | **cumprido em 2026-09-21**: 426/426 e 1.969/1.969 (`cargo test -p dartforge-frontend --test corpus -- --ignored`, 1,7 s) |
| `elements` | todo `dart:x` de `libraries.json` (`dartdevc`) carrega, com patches aplicados, e toda referência de nome do SDK e do corpus resolve | concluído (36 libs, 179 unidades, 1.525 classes, 269/269 supertipos resolvidos, 0 erros no elements) |
| `types` | SDK e corpus sem erro de tipo; testes negativos do `analyzer` reprovam onde ele reprova | — |
| `lowering` + `emit_js` | `dart:core` compilado do SDK executa `print('olá')` no Node; depois os pontos de entrada do corpus com a mesma saída do `dart compile js` | — |

A medição de latência e memória continua a do incremento 26 (alocações por
compilação, `live_bytes` em platô), aplicada a cada fase quando ela existir.

## 7. O que ainda não está decidido

* Representação da IR média: enum própria ou reaproveitar a `Ast` com tabelas
  laterais até a emissão exigir mais. A decisão vem quando `types` existir.
* `async` por geradores JS versus máquina de estados própria: geradores são
  mais simples e o DDC os usa; a máquina de estados do dart2js é mais rápida.
  Começa por geradores; troca só com medição.
* Source maps: obrigatórios para o modo de desenvolvimento; formato decidido
  na emissão.

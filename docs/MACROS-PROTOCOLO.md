# Macros: hospedeiro, executor nativo e o protocolo `dfmacro/1`

Contrato (P0) dos passos P8–P11 do plano "Dart 3.7–3.13 e macros". Nada disto
está implementado nesta rodada; o que está aqui é o que a implementação vai
seguir. **Substitui** `docs/MACROS-ARQUITETURA.md` §"herdar as fases,
descartar o transporte" e a Q4 (macros de usuário fora do contrato) — decisão
D8. Decisões de base em [`VERSOES-LINGUAGEM.md`](VERSOES-LINGUAGEM.md) §0.

## 1. O que se implementa

* **API e comportamento observável**: o da 1ª geração que o SDK 3.6.2 executa
  com `--enable-experiment=macros` (D5) — `package:macros` 0.1.3-main.0 /
  `_macros` 0.3.3. Caso de aceite: `@JsonCodable` do `package:json` 0.20.4.
  **A API é reescrita por nós** (D3), seguindo
  `references/dart-language/working/macros/feature-specification.md` e usando
  o `_macros` 0.3.3 só como referência de forma; não vendorizamos `_macros`.
* **Oráculo**: a VM 3.6.2 (saída do programa) e o texto de augmentation que o
  3.6.2 gera (obtido pelo LSP, `dart/textDocumentContent` com
  `dart-macro+file:`), **byte a byte**. dart2js/DDC 3.6.2 nunca executaram
  macros; a web confere a forma com o DDC 3.13.4 sobre o texto gerado como
  *part* (`augmentations,enhanced-parts`). Macro em biblioteca ≥ 3.7 (D6):
  permitida, e o oráculo é a expansão em texto executada no 3.13.4.
* **Divergências spec × 3.6.2** (vale o 3.6.2, D9): sem embrulho de corpo nem
  `augmented()`; sem `evaluate(Code)`/`DartObject` nem `Resource` (espaço
  reservado no protocolo); aspect macros fora.

## 2. Regra governante (PLANO.md, 2026-09-23) — como este desenho a cumpre

1. **Custo zero para quem não usa.** Sem aplicação de macro, a compilação é a
   de hoje: nenhum processo iniciado, nenhuma passada extra. A detecção
   reaproveita o que já existe: as anotações estão na árvore, e **só uma
   anotação que resolve para o construtor de uma classe declarada com
   `macro`** liga o hospedeiro. Sem `--enable-experiment=macros`, `macro
   class` nem é aceita (D5), então um projeto 3.6 comum não paga nem a
   consulta. Portão no CI: tempo do corpus e latência de edição do
   `dartforge dev` (227 ms de corpo no `new_sali/core`) não mudam.
2. **Uma infraestrutura só** para executar Dart em tempo de compilação:
   macros e builders do ecossistema (Fase 4 do `BUILD-RUST.md`) usam o mesmo
   executor, o mesmo protocolo e os mesmos caches.
3. **Executor persistente e quente**: um processo por sessão (`dev`, `lsp`;
   no `compile-js` avulso, iniciado sob demanda e só se houver aplicação),
   reaproveitado entre fases e edições. A macro é compilada **uma vez**, com o
   resultado em cache por `blake3(fontes do fecho da macro + versões + versão
   do DartForge + ABI)` — nunca recompilada por ciclo.
4. **Só reexecuta o que mudou**: cada execução registra o que consultou
   (introspecção, com o *digest* de cada resposta); o resultado é reusado
   enquanto a chave e os digests não mudarem. Editar o corpo de um método que
   nenhuma macro consultou não executa macro nenhuma (§6).
5. **Paralelo onde a spec permite**: aplicações independentes na mesma fase
   (a spec garante que a ordem não é visível dentro da fase) rodam em
   paralelo no executor, com saída determinística (o texto é montado na ordem
   da spec, §4, não na de conclusão).
6. **Nada de contabilidade em disco**: o texto gerado vive em memória
   (`crates/elements/src/gerado.rs`); o disco só recebe o que o usuário pedir
   (`--emitir-augmentations`).
7. **Orçamentos medidos**: partida a frio do executor uma vez por sessão;
   edição que reexecuta uma macro com executor quente na casa das dezenas de
   ms além da edição comum; build sem mudança na casa das dezenas de ms. Cada
   número entra no ESTADO medido, e regressão reprova no CI.

## 3. Quem executa: o executor nativo (D4)

A macro é Dart do usuário. Ela é compilada pelo **nosso** `emit_native` e
executada pelo **nosso** executor — o JIT ORCv2 sobre o mesmo IR
(`crates/jit`) ou o AOT — num **processo separado** (sandbox da spec,
`:1226-1234, :1302-1395`), falando `dfmacro/1` por stdio. Não há Node, motor
JS embutido nem runtime de terceiros. `int` tem a semântica da VM (64 bits):
a D7 some.

Consequência de ordem: executar macros depende de o nativo cobrir o que a API
usa (`async`/`Future`, coleções, strings, closures, `dart:convert`). Tudo o
que não depende do executor (augmentations escritas à mão, P7; o hospedeiro e
o protocolo com um executor falso que responde de um JSON gravado) anda
antes.

Bibliotecas permitidas no fecho de uma macro (`:1329-1350`):
`dart:{async,collection,convert,core,math,typed_data}`; importar outra é erro
de compilação, apontando a importação.

* **Rust (`crates/macros_host`)**: descoberta das aplicações, SCCs de
  bibliotecas (`elements/src/ciclos.rs`, Tarjan sobre imports/exports/parts;
  macro não se aplica no SCC em que é definida), ordem e fases, espera da
  fase 2, consultas servidas de `elements`/`types`, *merge* do texto
  devolvido (parse como unidade de augmentation e fusão no `elements`,
  [`AUGMENTATIONS.md`](AUGMENTATIONS.md)), cache e digests. Trait
  `Executor { iniciar, instanciar, executar_fase, montar, encerrar }`; a
  única implementação é a nativa.
* **Processo da macro**: *bootstrap* gerado (mapa URI → classe → construtor
  por *tear-off*, sem mirrors), transporte stdio, fachada da API que
  implementa as introspecções sobre o modelo recebido, execução das três
  fases e montagem do texto de augmentation **dentro do processo**.

## 4. Fases e ordem (spec de macros)

* **Types** (`:564-585`): só nomes; `resolveIdentifier`; `declareType`,
  `appendInterfaces`/`appendMixins`/`extendsType`. Não gera `macro class`.
* **Declarations** (`:587-613`): `fieldsOf`/`methodsOf`/`constructorsOf`/
  `valuesOf`/`typesOf`/`typeDeclarationOf`/`resolve`, `declareInType`/
  `declareInLibrary`. A introspecção de tipo do mesmo ciclo **espera** as
  aplicações nele: grafo de espera explícito com detecção de ciclo por DFS;
  um ciclo lança `MacroIntrospectionCycleException` e **toda** consulta
  seguinte ao mesmo alvo também lança (registrado por alvo, `:602-607`).
  Ordem de execução como o CFE 3.6.2: por classe, na ordem da hierarquia, com
  a augmentation de cada classe montada antes da próxima.
* **Definitions** (`:615-627`): `declarationOf`, `inferType`,
  `topLevelDeclarationsOf`, `build*`. Construtor padrão só aparece aqui.
* **Ordem de aplicação** (`:250-286`): interna antes da externa; na mesma
  declaração, da direita para a esquerda; aplicação gerada por macro expande
  logo depois, na mesma fase. Função pura da árvore, com teste por regra.
* **Texto** (`:288-487`): Regras 1–4 (fusão por tipo, ordem fase → aplicação
  → offset, uma linha em branco, tipo novo separado da sua augmentation). É o
  critério de determinismo e o oráculo byte a byte, na forma `part of` +
  `import … as prefixN`.
* Listas de introspecção **ordenadas por nome no host** (`:720-733`), com o
  digest sobre a lista ordenada: reordenar membros não reexecuta macro.
* Erros: declaração gerada que sombreia identificador já resolvido durante as
  fases (`:960-965`), colisão com declaração existente (`:912-915`), macro no
  mesmo SCC — todos apontam a anotação.

## 5. O protocolo `dfmacro/1`

* **Handshake**: o hospedeiro envia `{protocolo: "dfmacro/1", versao_dartforge,
  abi}`; o executor responde com o mesmo nome e a lista de macros do bundle.
  Nome ou ABI diferente encerra com diagnóstico claro.
* **Quadros**: u32 big-endian com o tamanho, seguido de JSON em UTF-8
  (formato binário só com medição que o justifique).
* **Mensagens** (`id` crescente por remetente; toda pergunta tem resposta):
  * `carregar {bundle, macros}` → `carregado {macros}`;
  * `instanciar {macro, construtor, argumentos}` → `instancia {id}` —
    argumentos só literais, `Code` e `TypeAnnotation` (`:1236-1300`);
  * `fase {tipo: types|declarations|definitions, instancia, alvo,
    modelo_inicial}` → `resultado {codigo_estruturado, diagnosticos,
    excecao}`; `modelo_inicial` já traz o alvo e os membros diretos
    (**pré-busca**: o `@JsonCodable` não faz ida e volta);
  * `consulta {id, tipo, …}` (do executor para o hospedeiro) →
    `resposta {id, modelo | erro}` — `resolveIdentifier`, `typeDeclarationOf`,
    `fieldsOf`, …, `inferType`, `resolve`, `isSubtypeOf`/`isExactly`;
  * `montar {biblioteca, resultados}` → `texto`;
  * `encerrar`.
  * Reservados: `avaliar {codigo}` (`evaluate`) e `recurso {uri}` (`Resource`,
    com dependência registrada, `:1625-1643`).
* **Modelo**: no estilo do `dart_model` da 2ª geração — `QualifiedName`
  (`uri#escopo.nome`), tipos estruturados, listas ordenadas, `digest` por nó;
  só declarações (sem corpos), salvo o que `inferType` devolve, que é
  resposta registrada.

## 6. Cache e incrementalidade

* **Chave de uma aplicação** = hash(identidade do bundle da macro,
  argumentos da anotação, digest do alvo no modelo). **Valor** = resultado
  estruturado + as consultas feitas, com o digest de cada resposta.
* Na reanálise o hospedeiro recalcula em Rust o digest do alvo e das respostas
  registradas: tudo igual → reutiliza **sem falar com o executor**. Uma edição
  de corpo não altera nada disso; o caminho de 227 ms continua com **zero**
  execuções de macro, e o relatório do `instrument` ganha o contador
  `macros_executadas` para provar isso.
* Mudou a API → reexecuta só as aplicações cujos digests mudaram, em ordem de
  fase; o texto final é refundido e a biblioteca aumentada entra no
  `hash_api_publica` pelo texto gerado.
* Aproveitado da trilha velha: a expansão atômica (falha não publica árvore
  parcial), o remapeamento de diagnóstico para a anotação
  (`crates/macros/src/lib.rs`, `ExpansionReport::remap`) e os limites do cache
  LRU. **Não** aproveitado: macros escritas em Rust como mecanismo, o
  `@JsonCodable` sem import e com semântica diferente, `@DataClass` com API
  inventada e a árvore de subconjunto.

## 7. Corpus de macros

`corpus/macros/<caso>/` é um pacote com `pubspec.yaml` e
`analysis_options.yaml` (`enable-experiment: [macros]`); referências
`saida.txt` (VM 3.6.2) e `lib/*.augmentation.dart` (texto do LSP 3.6.2).
Critério: saída e augmentation byte a byte; o determinismo
(`--trabalhadores 1,4,8`) cobre as augmentations. Casos mínimos:
`@JsonCodable` (simples, aninhado, `List`/`Set`/`Map`, anulável, superclasse
serializável, `DateTime`), `JsonEncodable`/`JsonDecodable`, os diagnósticos do
`json`, macros locais das três fases com as Regras 1–4, ciclo de fase 2,
macro no mesmo SCC, sombreamento, aplicação gerada por macro e `inferType`.

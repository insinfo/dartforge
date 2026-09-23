# Versões de linguagem: do piso 3.6 à 3.13

Contrato dos passos P0–P6 do plano "Dart 3.7–3.13 e macros" (aprovado em
2026-09-23). **3.6.2 é o piso, não o teto**: o DartForge acompanha as versões
novas da linguagem respeitando a versão de cada biblioteca, como o CFE faz, e
os recursos novos são conferidos contra o SDK oficial que os implementa
(3.13.4), não contra exemplos informais. Macros e augmentations têm contratos
próprios: [`MACROS-PROTOCOLO.md`](MACROS-PROTOCOLO.md) e
[`AUGMENTATIONS.md`](AUGMENTATIONS.md) (passos P7+, fora desta rodada).

Fontes normativas, por ordem de precedência quando divergem:

1. o comportamento do SDK **3.13.4** (VM e `dartdevc`), executado em
   programas mínimos — é o oráculo do corpus;
2. as propostas aceitas em `references/dart-language/accepted/3.{7,8,10,12,13}/`;
3. o rascunho formal `references/dart-spec/DartLangSpecDraft.txt` (6ª edição,
   2.13-dev), que ainda não traz nenhum destes recursos e vale para a base.

## 0. Decisões do proprietário (2026-09-23)

| | decisão |
| --- | --- |
| **D1** | `dart:*` continua o do **3.6.2** (`dart_sdk.js` + `lib/` 3.6.2). API de biblioteca marcada `@Since` 3.7+ (por exemplo `Future.syncValue`, `int.oneBitCount`, `List.unmodifiableOf`) não existe aqui e dá diagnóstico de membro indefinido. Subir o runtime é passo posterior. |
| **D2** | A versão **corrente** da ferramenta é **3.13**: é a de um arquivo sem marcador e fora de pacote, e o teto dos marcadores. `--versao-linguagem x.y` a troca (o harness passa a versão do programa). |
| **D3** | A API de macros é **reescrita por nós** seguindo a especificação, com a 1ª geração (`_macros` 0.3.3) como referência de forma, sem vendorizar `_macros`. O critério byte a byte contra a augmentation do CFE 3.6.2 continua. |
| **D4** | O executor de macros é **auto-hospedado no nosso backend nativo**: a macro é compilada pelo `emit_native` e executada pelo nosso executor (JIT ORCv2 sobre o mesmo IR, `crates/jit`, ou AOT), num processo separado, falando `dfmacro/1` por stdio. Nada de Node, motor JS embutido ou runtime de terceiros. Tudo o que não depende do executor anda já. |
| **D5** | Macros exigem `--enable-experiment=macros`, como o oficial; augmentations escritas à mão exigem `augmentations,enhanced-parts`. |
| **D6** | Macros em biblioteca ≥ 3.7 são permitidas; o oráculo é a expansão em texto executada no 3.13.4 como *part*. |
| **D7** | Some (não há executor JS, logo não há `int` de 53 bits no executor). |
| **D8** | Este desenho substitui `docs/historico/MACROS-ARQUITETURA.md` (fases "herdadas, transporte descartado" e Q4). |
| **D9** | Segue-se o CFE 3.6.2 e a spec de augmentations 1.46: sem `augmented()`, sem embrulho de corpo. |

**Regra governante** (PLANO.md, "geração de código e macros: rápidas, e custo
zero para quem não usa"): o gating por versão e a detecção de recursos não
podem custar nada perceptível para um projeto 3.6 comum. A versão é resolvida
**uma vez por biblioteca**, no carregamento, e as fases só consultam um
conjunto de bits. Medido em §7.

## 1. Inventário 3.7–3.13

| ver. | recurso (`experimental_features.yaml`) | natureza | onde entra | o que os emissores veem |
| --- | --- | --- | --- | --- |
| 3.7 | curingas `_` (`wildcard-variables`) | escopo | `types` (escopo), `emit_js` (nome JS) | local sem nome; nenhuma forma nova |
| 3.7 | inferência usando bounds (`inference-using-bounds`) | inferência | `types/constraints.rs`, inferência de argumentos de tipo do `emit_js` | argumentos de tipo reificados diferentes |
| 3.7 | #56893: campo promovido a `Null` conta na alcançabilidade | fluxo | `types/flow.rs` | nada (só aceita/recusa programas) |
| 3.8 | elementos null-aware `?e`, `?k: ?v` (`null-aware-elements`) | sintaxe + tipos | parser (gating), `types`, `emit_js`; o nativo recusa | `CollectionElement::NullAwareExpression` / `MapEntry{null_aware_*}` |
| 3.9 | fluxo sólido (`sound-flow-analysis`) | fluxo | `types/flow.rs` | nada |
| 3.9 | getter/setter com tipos diferentes deixa de ser erro (`getter-setter-error`) | diagnóstico | `types` | nada |
| 3.10 | atalhos de ponto (`dot-shorthands`) | sintaxe + inferência | parser (`ExprKind::DotShorthand`), `types` (resolução), `emit_js`, `mundo`, `emit_native` | um caso por consumidor: "`.id` é `D.id`", `D` gravado em `Resolved` |
| 3.10 | tipo de retorno de gerador sem `Null` espúrio | inferência | `types/infer.rs` | tipo reificado |
| 3.12 | parâmetros nomeados privados (`private-named-parameters`) | nomes | parser (`Parameter::public_name`), consumidores do nome externo | o nome externo do parâmetro |
| 3.13 | construtores primários, `new`/`factory` sem o nome da classe, corpo `;` (`primary-constructors`) | sintaxe + elaboração + escopo | parser, que também elabora (`parser/declarations.rs`) | **membros comuns**: campos e um construtor (E) |
| 3.13 | quebra: `var`/`final` só em parâmetro declarante; `factory() {}` vira construtor | gramática | parser, decidido pela versão | — |
| 3.13 | #62889: ajuste de promoção | fluxo | `types/flow.rs` | nada |

Fora: `native-assets` (3.10) e `record-use` (3.13) são ferramentas de *hooks*,
não linguagem. A 3.11 não mudou a linguagem.

**Runtime.** Nenhum recurso exige runtime novo: o `dartdevc` 3.13.4 rebaixa
todos para construções que o `dart_sdk.js` 3.6.2 já tem (conferido lendo o JS
emitido de cada programa de prova: acesso estático comum, `new C.nome()`,
campos atribuídos no construtor, nomes JS únicos para curingas, parâmetro
nomeado com o nome público). O `dart.addRtiResources` que aparece no JS do
3.13.4 é do contrato novo do DDC, não dos recursos, e não é copiado.

## 2. Versão de linguagem por biblioteca

Regras (`accepted/2.8/language-versioning/feature-specification.md`):

* o marcador `// @dart = x.y` vem antes de qualquer token, é comentário de
  linha (não `///`, não dentro de `/* */`) e só o primeiro vale; a gramática é
  a do *scanner* do CFE (`abstract_scanner.dart`,
  `tokenizeLanguageVersionOrSingleLineComment`): espaços (não tabulações) em
  volta de `@dart`, `=` e da versão, nada depois dela;
* sem marcador, vale o `languageVersion` do pacote da biblioteca no
  `package_config.json` — `package:x/…` pertence a `x`; um arquivo pertence ao
  pacote cuja raiz (`rootUri`) o contém, a mais interna (então `bin/` e `test/`
  também); sem pacote ou sem `languageVersion`, a versão corrente (D2);
* uma *part* tem a versão da sua biblioteca; se a versão própria dela
  (marcador ou pacote) for outra, é erro (`LanguageVersionMismatchInPart`);
* marcador acima da corrente ou abaixo de 2.12 é erro, e a biblioteca fica na
  versão padrão (`LanguageVersionTooHighExplicit`/`TooLowExplicit`); pacote
  acima da corrente ou com `languageVersion` que não é `x.y` também é erro;
* `--enable-experiment` só vale para bibliotecas sem marcador cuja versão
  padrão é a corrente (`:159`);
* `dart:*` fica sempre no **piso 3.6** (D1).

Implementação:

| peça | onde |
| --- | --- |
| `LanguageVersion`, `enum Feature` (tabela `enabledIn` transcrita do yaml, com teste que a confere), `LibraryFeatures` (8 bytes, `Copy`, `tem(f)` é um `&` de bits), `marcador_versao` (varredura pura do cabeçalho) | `crates/frontend/src/features.rs` |
| `parse_com(fonte, nomes, features)`; o `parse` de sempre é `parse_com(…, LibraryFeatures::atual())`; `Parser::exigir(f, span)` registra "o recurso 'x' exige a versão de linguagem X.Y" | `crates/frontend/src/parser/mod.rs` |
| a versão de cada unidade, **antes do parse**, a partir da fonte já lida e do `package_config`; `Library::features` e `Unit::features` | `crates/elements/src/load.rs` (`Versao`, `features_da_unidade`), `model.rs` |
| a que pacote pertence um arquivo (`root_dirs`, `pacote_da_biblioteca`) | `crates/elements/src/config.rs` |
| a versão corrente e os experimentos: `SdkLayout::versao_corrente`/`experimentos`, `Linguagem::ler_opcao` (`--versao-linguagem x.y`, `--enable-experiment=a,b`) em `compile-js`, `dartforge-jsprod` e `CompileOptions::versao_linguagem` do nativo | `crates/elements/src/sdk.rs` |
| SDK no piso; o cache do SDK leva `features.rs` na chave | `crates/elements/src/sdk_cache.rs` |
| sessão residente: uma unidade guardada só é reaproveitada se a versão recalculada (marcador da fonte guardada + `package_config` de agora) for a mesma com que foi analisada; a versão entra nos dois hashes da biblioteca (mudar o `languageVersion` invalida sem mudar a fonte) | `crates/elements/src/unidades.rs` (via `load_unit`), `crates/dev/src/hashes.rs` |

O parser aceita sempre o **superconjunto** (princípio do crate); um recurso
desligado é diagnóstico, não erro de sintaxe. A versão **decide** só onde a
gramática diverge entre versões (§4.5): `factory() {}` e `var`/`final` em
parâmetro.

## 3. Diagnósticos que são erro

Os diagnósticos de `elements` (carga e sintaxe, inclusive os de versão)
abortam a compilação; os de `types` são avisos, porque a inferência ainda tem
lacunas. Os erros dos recursos novos — o que o CFE recusa e um programa
negativo do corpus precisa ver recusado — saem de três lugares, sempre com a
posição, e todos abortam `compile-js`, `dartforge-jsprod` e o nativo (a lista
não cresce por conveniência: cada um tem programa negativo no corpus
conferido contra o CFE, na mesma linha):

* **parser** (sintáticos): recurso desligado pela versão; `var`/`final` em
  parâmetro comum na 3.13; nomeado privado que não inicializa campo, sem nome
  público ou colidindo com outro parâmetro (inclusive `super._x`); construtor
  primário com parte `this` sem cabeçalho, duas partes `this`, construtor
  generativo não redirecionador no corpo, `const` com corpo na parte `this`,
  parte `this` com `=>`/`async`, `covariant` sem `var`, `var` em extension
  type, `const .x` sem argumentos;
* **`types`**, com o prefixo [`codes::ERRO_DE_LINGUAGEM`]: ler `_` quando só
  curingas o declaram (`Undefined name '_'`);
* **`emit_js`** (`Ctx::erros`), com o mesmo prefixo: atalho de ponto sem
  contexto que denote declaração, ou sem membro com esse nome (`No type was
  provided to find the dot shorthand 'x'`). É o emissor que tem a inferência
  completa do contexto; `types` só registra `D` quando a acha, sem acusar
  nada (a inferência dele não leva contexto a todo lugar, e um erro falso
  abortaria um programa válido). Emissão especulativa (`type_of`) não conta.

## 4. Contrato de cada recurso

### 4.1 Curingas `_` (3.7)

Spec: `accepted/3.7/wildcard-variables/feature-specification.md`. Em
biblioteca ≥ 3.7, **não ligam nome**: variável local (inclusive de `for` e de
`for-in`, em statement e em elemento de coleção), parâmetro de qualquer
função (topo, método, construtor, função local, expressão de função, tipo de
função), `this._` (inicializa o campo `_` e não liga nada), `super._`
(encaminha e não liga nada), `catch (_, _)`, função local chamada `_`,
parâmetro de tipo `<_>` de classe, função e typedef, e prefixo de import `as _`
(as extensões dele continuam visíveis). Padrões já eram curinga em toda
versão. **Continuam ligando**: topo, membro, tipo (`var _ = 1;` de topo, campo
`_`). O inicializador é avaliado. Ler `_` resolve para fora ("wildcards do not
shadow"): campo, topo ou import; se nada chamado `_` estiver visível, é erro.
Em biblioteca < 3.7, `_` é nome comum (`// @dart=3.6`).

* `types`: `ScopeStack::curinga` (o símbolo `_` quando a biblioteca tem o
  recurso); local e parâmetro curinga ganham `LocalId`, mas não entram na
  busca por nome.
* `emit_js`: o nome JS de um curinga é único por função (`t$wN`: o prefixo
  `t$` é reservado aos temporários e nomes de usuário que começam com ele
  ganham `$` no fim, então não há colisão), e ele não entra no escopo; um
  `this._`/`super._` curinga guarda o nome JS pelo intervalo do parâmetro para
  o construtor inicializar o campo/encaminhar. Parâmetros de tipo curinga de
  função genérica também ganham nome único.
* `emit_native`: **não muda** — `_` continua ligando nome no nativo. Só é
  observável quando o programa lê um `_` de fora com um curinga no caminho,
  e o nativo não roda o `corpus/moderno`; o `this._`/`super._` do
  construtor nativo lê o parâmetro pelo nome, e desligá-lo exigiria o mesmo
  registro por intervalo do `emit_js`.

### 4.2 Elementos null-aware (3.8)

Spec: `accepted/3.8/null-aware-elements/feature-specification.md`. A árvore
já os representava (`CollectionElement::NullAwareExpression`,
`MapEntry{null_aware_key, null_aware_value}`); faltavam o gating (parser,
§2), o `Set`, o contexto anulável, o curto-circuito do **valor** quando a
chave null-aware é `null` (conferido na VM: o valor não é avaliado) e o
nativo. Tipos: contexto `Ps?` para o operando, elemento `NonNull(U)`.
Constante quando o operando é constante. `emit_native` **recusa** o
elemento null-aware com diagnóstico explícito: o literal nativo é uma
alocação com elementos fixos (`AllocList`/`AllocMap`), sem inserção
condicional — a mesma lacuna de `if`/`for`/spread em coleção no nativo, e
fica junto dela.

### 4.3 Atalhos de ponto (3.10)

Spec: `accepted/3.10/dot-shorthands/feature-specification.md`.

* **Árvore**: `ExprKind::DotShorthand { name, const_ }` é o *head* — `.id`,
  `.new`, `const .id`, `const .new`; os seletores seguintes são os de sempre
  (`.parse('1')` é `Call { target: DotShorthand }`). Padrão constante `case
  .x` e `case const .x(…)` são `PatternKind::Constant(DotShorthand…)`. `.`
  não inicia statement de expressão.
* **Contexto** (*shorthand context*): o tipo de contexto da **cadeia inteira**
  de seletores vai para o head; `==`/`!=` com o operando direito sendo
  exatamente um atalho usam o tipo estático do esquerdo; padrão constante e
  `== .x` usam o tipo do valor casado. O contexto denota uma declaração `D`
  quando é `C`/`C<…>` de classe, mixin, enum ou extension type, ou `S?`/
  `FutureOr<S>` com `S` denotando `D`.
* **Contrato com os consumidores** (T): `types` grava, para cada nó
  `DotShorthand` cuja declaração acha, `Resolved::Element(Element::Class(D))`
  (e `Resolved::Constructor` na chamada `.nome(args)` que é construção).
  Cada consumidor trata `.id` exatamente como `D.id` (e `.new` como
  `D.new`/`D(…)`), sem instanciar `D` com os argumentos do contexto — a
  inferência do construtor usa o contexto da chamada, como
  `List<int> l = .filled(2, 0)` → `List<int>.filled`. `emit_js` refaz a mesma
  derivação sobre o seu próprio `Ty` (ele não lê `Resolved`): o nó mais
  externo da cadeia registra o contexto da raiz, `==` e o padrão `== .x`
  registram o tipo da esquerda/do valor casado, e o `=>` de função `async`
  registra `FutureOr<T>`. `mundo` lê `Resolved`; nos padrões de `switch`,
  onde `types` não chega, marca vivo `nome` em toda declaração que o tem
  (mais vivo, nunca menos). `emit_native`: a construção sai pela
  `Resolved::Constructor`; atalho como valor e método estático por atalho
  caem no "não suportado" dele.
* Erros: sem contexto que denote declaração; membro inexistente (§3).

### 4.4 Parâmetros nomeados privados (3.12)

Spec: `accepted/3.12/private-named-parameters/feature-specification.md`. Em
biblioteca ≥ 3.12, um parâmetro **nomeado** que inicializa (`this._x`) ou
declara (`var _x` num construtor primário) campo privado pode ter nome
privado: o nome **externo** (assinatura, chamada, `Function.runtimeType`) é o
público correspondente (`x`); o local, visível só na lista de inicializadores,
continua `_x`; no corpo `_x` é o campo. Sem nome público (`_`, `_2x`, `__x`)
ou colidindo com outro parâmetro (`p` ou `n`) é erro. Qualquer outro
parâmetro nomeado privado continua erro, em toda versão. `super.x` encaminha
com o nome público; `super._x` é erro.

* `Parameter::public_name: Option<Name>`, preenchido pelo parser. Os
  consumidores do nome **externo** usam `public_name.unwrap_or(name)`
  (`ast::Parameter::nome_externo`); os do nome **local/campo**, `name`.

### 4.5 Construtores primários (3.13)

Spec: `accepted/3.13/primary-constructors/feature-specification.md`. O
contrato antigo de `docs/historico/IMPLEMENTACAO-25.md` §4 ("`Tipo nome` declara campo
final") **contradiz** a spec (`:1000-1002`: parâmetro sem `var`/`final` é
simples, não declara nada) e o oráculo (`class Nome(String cru)`); está
corrigido lá e vale este.

* **Gramática** (`:506-649`): `class`/`enum`/`extension type` com
  `<primaryConstructor>` no cabeçalho (`const`? nome, parâmetros de tipo,
  `.id`/`.new`? e a lista declarante), corpo `;` para class/mixin/extension
  type; membro `this : inits? corpo?` (a parte de corpo); `new id?` e
  `factory id?` no corpo, com o nome da classe implícito; `factory C(` com
  `C` = a classe declara o construtor `C` (não `C.C`). Em biblioteca ≥ 3.13,
  `factory() {}` é construtor, e `var`/`final` só é permitido em parâmetro de
  construtor primário (`Can't have modifier 'final' here`); < 3.13 é o
  contrário (`factory` é nome de método).
* **Elaboração (E)** — derivação D → D2 (`:926-1025`), feita
  **sintaticamente** pelo parser ao fim de cada `class`/`enum`
  (`parser/declarations.rs`, `elaborar_construtor_primario`), na arena da
  própria unidade: cada `var`/`final T p` vira um campo `T p;`/`final T p;`
  (o `covariant` passa ao campo) e o parâmetro vira `T this.p` com o mesmo
  default; `this.p`, `super.p` e parâmetros simples são copiados; o
  construtor k2 recebe o nome (`C` ou `C.id`), o `const` do cabeçalho (ou do
  `enum`), a lista de inicializadores e o corpo da parte `this`, e ocupa o
  lugar dela (os campos vêm antes dos membros do corpo). Tipo omitido num
  parâmetro declarante (`:945-962`: o do getter herdado de mesmo nome,
  senão o do default, senão `Object?`): a parte sintática é feita aqui —
  default literal dá o tipo dele, `null` ou nenhum dá `Object?` — só quando
  a classe não declara supertipo; com supertipo, o campo fica sem tipo, e a
  override inference de campo é do `types` (P6). Os três consumidores veem
  **código comum**; nenhum emissor sabe que houve construtor primário.
* **Extension type**: a representação já era o construtor primário dele
  (3.3); a 3.13 só acrescenta `final` (e recusa `var`) e o corpo `;`.
* **Escopo primário** (`:817-890`): os inicializadores de campo **não-`late`**
  e a lista de inicializadores veem os parâmetros (o `this.p` elaborado é o
  parâmetro, como num construtor comum); o corpo da parte `this` vê só os
  parâmetros simples — `p` declarante é o **campo**, que é exatamente a regra
  de `this.p` num construtor comum. O que falta a um construtor comum é o
  primeiro ponto: `ClassDecl`/`EnumDecl::primary_constructor` marca o k2, e o
  `emit_js` emite os inicializadores de campo não-`late` de uma classe com
  construtor primário **pelo emissor do próprio k2**, com os parâmetros em
  escopo, dentro do construtor, como o DDC (`this[up] = cru.toUpperCase()`);
  `late` continua preguiçoso e fora dele. `types` não mudou aqui: o tipo de
  um campo sem tipo que lê parâmetro fica `dynamic` para ele (aviso), sem
  efeito na saída.

### 4.6 Inferência e fluxo (3.7–3.13)

> **Passou ao dono de `crates/types`** (decisão de 2026-09-23, durante a
> rodada): o contrato abaixo continua valendo, e os programas 350–352 do
> `corpus/moderno` ficam em `PENDENTES` até ele.

* **Bounds (3.7)**: `design-document.md` de `inference-using-bounds`. Ao
  resolver uma variável de tipo sem solução pelas restrições, o bound declarado
  entra como restrição (com a própria variável substituída pela solução
  parcial) antes de desistir: `f<X extends A<X>>(C())` infere `X = B`
  (`C <: B <: A<B>`), onde < 3.7 era erro.
* **Fluxo sólido (3.9)** e #56893/#62889: afetam só quais programas são
  aceitos (atribuição definitiva, alcançabilidade); como o DartForge não recusa
  programa por atribuição definitiva, o efeito observável é nenhum. O par
  `s38`/`s313` do corpus confere: aceito na 3.13, recusado pela VM na 3.8.
* **getter/setter (3.9)**: não havia o diagnóstico; nada muda.
* **Gerador (3.10)**: `f() sync* { yield 1; return; }` infere `Iterable<int>`
  nas duas versões na VM 3.13.4 (a diferença era do analyzer); o programa do
  corpus fixa o comportamento.

## 5. Harness, corpus e CI

* `crates/diferencial`: **dois SDKs de oráculo**, `DARTFORGE_DART_SDK` (o
  piso, 3.6.2) e `DARTFORGE_DART_SDK_3_13` (padrão
  `D:/DartSDKs/3.13.4/dart-sdk`). Cada SDK tem o seu `dart`, o seu
  `dartdevc` e o **seu** `dart_sdk.js` (`runtime/ddc/dart_sdk.js` e
  `runtime/ddc/3.13.4/dart_sdk.js`, gerados por `gerar-dart-sdk.ps1 -Sdk …
  -Saida …`). O cache dos oráculos do piso mantém os rótulos de sempre; os do
  3.13.4 levam a versão.
* Cabeçalhos do programa: `// @dart=x.y` (o marcador oficial, vale para os
  três executores), `// requer-dart: x.y` (a versão corrente que o programa
  exige: oráculo = menor SDK que cobre isso e o marcador; o DartForge recebe
  `--versao-linguagem` com a versão desse SDK), `// erro-de-compilacao`
  (negativo: os três têm de recusar; a saída comparável é "erro de compilação
  na linha L", L = linha do primeiro erro de cada um), `// experimentos: a,b`.
* `corpus/js` fica intacto (3.6.2, versão 3.6). `corpus/moderno` tem um
  programa por recurso e por regra da spec, mais os pares de gating. Um
  `PENDENTES` no diretório do corpus lista o que ainda falha por recurso não
  implementado: pendente que falha não reprova; pendente que passa reprova
  (a lista só encolhe).
* CI (`.github/workflows/pesado.yml`): a ação `.github/actions/dart` instala
  o 3.6.2 (`setup-dart`) e, com `sdk-313: true`, o 3.13.4 por zip, com cache
  pela versão, exportando `DARTFORGE_DART_SDK_3_13`. Job `moderno`: o
  `corpus/moderno` em desenvolvimento e produção, código 0 = todos passam
  (com `PENDENTES`).

## 6. Fora desta rodada

* Macros e augmentations (P7+).
* Inferência e fluxo 3.7–3.13 (P6, §4.6): com o dono de `crates/types`.
* O adaptador dos 220 testes de
  `references/dart-sdk/tests/language/{wildcard_variables,…}` (§6.2 do
  plano): dependem de `package:expect`, que importa `dart:io` via
  `package:smith`.
* A biblioteca de plataforma 3.13 (D1).
* **Membros de extension type** (Dart 3.3, anterior à rodada): `Id(21).dobro`
  sai `21.dobro` no `emit_js` e `i.tri()` falha — só o interop JS de extension
  type funciona. O programa 347 (a gramática 3.13 de extension type) fica em
  `PENDENTES` por isso.
* No nativo: curingas (`_` liga nome), elementos null-aware (recusa explícita)
  e atalho de ponto que não é construção (§4.1–4.3).

## 7. Placar e medições

Preenchido com números medidos ao fim da rodada.

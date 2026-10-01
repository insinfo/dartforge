# JS de produção: tamanho e tempo do dart2js

Especificação escrita depois de medir e antes de implementar, como pede o
método do projeto. O plano anterior (`docs/JS-PRODUCAO-SDK-PROPRIO.md`)
tirou o `dart_sdk.js` do arquivo. A meta deste é a do dono: nos projetos
reais, o **mesmo tamanho** do `dart compile js -O4` e o mundo fechado
**na mesma ordem de tempo** dele.

Referências lidas, com arquivo:linha citado onde a regra é usada:

* dart2js: `E:\references\dart-sdk\pkg\compiler\lib\src\` (`enqueue.dart`,
  `resolution/enqueuer.dart`, `universe/`, `inferrer/`, `js_backend/`,
  `js_emitter/`);
* TFA da VM: `E:\references\dart-sdk\pkg\vm\lib\transformations\type_flow\`
  (`rta.dart`, `analysis.dart`, `transformer.dart`,
  `table_selector_assigner.dart`);
* minificadores em Rust: `E:\references\oxc` (0.150; usamos 0.152),
  `E:\references\swc`, `E:\references\rolldown` e `E:\references\rspack`.

---

## 1. Medidas (2026-10-01)

### 1.1 Tamanho e tempo

| projeto | nosso, bruto | gzip | dart2js -O4, bruto | gzip | nosso, tempo | dart2js, tempo |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `new_sali/frontend` | 24.895.868 | 3.578.704 | 7.616.039 | 2.025.116 | 14 min 22 s | 57 s |
| `limitless_ui/example` | 14.043.321 | 2.074.627 | 4.511.811 ¹ | 1.195.800 | 8 min 23 s | — |
| `01_print` | 257.738 | 46.988 | 34.929 | 11.551 | 0,3 s | 2,6 s |

¹ O build oficial (`build_web_compilers --release`).

Tempo do nosso no `new_sali`, por fase (`dartforge-jsprod`, soma das
rodadas do ponto fixo, 6 rodadas):

| fase | tempo |
| --- | ---: |
| mundo fechado (`dartforge_mundo::calcular_com`) | **707 s** |
| verificador do texto (`verificar::conferir`) | **169 s** |
| emissão | 19 s |

No `limitless_ui`: mundo 460 s, verificador 44 s, emissão 8 s.

### 1.2 Do que é feito o arquivo do `new_sali` (24,9 MB minificado)

Por categoria de token (contado com o *tokenizer* do acorn,
`E:\dftemp\jsprod\tokens.js`):

| categoria | bytes | % |
| --- | ---: | ---: |
| nomes de propriedade (`.x`) | 6.326.336 | 25,5 |
| receitas rti em *string* (`lib\|Classe<…>`) | 3.445.312 | 13,9 |
| JSON das regras rti (`addRules`) | 1.688.999 | 6,8 |
| outras *strings* | 4.140.217 | 16,7 |
| pontuação | 4.284.191 | 17,3 |
| identificadores (já minificados pelo `oxc`) | 2.695.561 | 10,9 |
| palavras-chave | 2.162.511 | 8,7 |
| números | 340.791 | 1,4 |

As propriedades mais caras:

| nome | bytes | o que é |
| --- | ---: | --- |
| `privateName` | 410.448 | `dart.privateName(lib, "x")`, uma chamada por nome privado |
| `CharacterCategory` | 296.010 | enum do `pdf_plus` (tabelas bidi) |
| `LateError` | 133.980 | `late` |
| `isDevToolsEnabled` | 118.746 | ngdart; `false` em todo o programa |
| `$constCache` | 106.920 | cache de constantes por biblioteca |
| `prototype` | 85.440 | |
| `addRtiResources` | 73.728 | |
| `Inspector` | 73.560 | ngdart; só alcançável sob `isDevToolsEnabled` |
| `setLibraryUri` | 64.428 | |

As receitas são longas porque cada classe é nomeada pela biblioteca
inteira:
`new_sali_frontend__src__modules__protocolo__components__…|Classe`.

Por pacote, no arquivo **sem** minificar (59,4 MB; `E:\dftemp\jsprod\pacotes.js`):

| o quê | KB | % |
| --- | ---: | ---: |
| templates do ngdart (`*.template.dart`) | 23.861 | 40,2 |
| código dos pacotes e do app | 16.058 | 27,0 |
| campos de topo e estáticos (`defineLazy`/`copyProperties`) | 11.399 | 19,2 |
| SDK (`dart_rti` sozinho: 3.426) | 4.789 | 8,1 |
| metadados (`set*Signature`, `addRtiResources`, `setLibraryUri`) | 2.908 | 4,9 |

Um template típico (`incluir_cgm_page.template.dart`) mostra o que o
dart2js não paga:

* `if (L$ngdart__src__devtools.isDevToolsEnabled) { … Inspector … }` em toda
  ligação. O campo só é escrito por `enableDevTools()`, que o programa
  nunca chama. A inferência do dart2js prova que vale `false` e apaga o
  bloco (`inferrer/engine.dart:850-884`);
* um par `get`/`set` por campo `late`, com `LateError.fieldNI`/`fieldAI`;
* o nome do pacote repetido em cada símbolo privado, receita e
  `setLibraryUri`.

---

## 2. Por que o mundo fechado leva minutos

A fila do `Motor` (`crates/mundo/src/lib.rs:1256-1270`) já processa cada
item uma vez. O tempo vai para três lugares:

1. **A regra de cone varre todas as classes.** `alcanca(membro, cone)`
   (`lib.rs:789-798`) monta, a cada chamada, a lista de **todas** as
   classes instanciadas e testa `e_subtipo` duas vezes em cada uma. Quem a
   chama:
   * `novo_seletor_com_receptor`, a cada uso de membro com receptor de tipo
     conhecido, para **todos** os pendentes do nome (`lib.rs:728-752`);
   * `seletor_para`, para cada membro de cada classe instanciada
     (`lib.rs:1146-1166`).

   É usos × pendentes × classes instanciadas, e é quadrático. O dart2js
   faz o mesmo casamento com custo constante amortizado:
   `_registerNewSelector` só age quando o conjunto de receptores **cresce**
   (`universe/resolution_world_builder.dart:541-560`), `_processSet`
   percorre só os pendentes daquele nome e os remove (`:779-797`), e o
   `_LiveSet` guarda, por par (dono, cone), se há classe viva que herda de
   um e implementa o outro, calculado uma vez
   (`universe/class_hierarchy.dart:911-1016`). A pré-passada RTA da TFA
   empurra cada seletor uma vez para cada subtipo alocado (`rta.dart:147-180`).
2. **O par (nome, cone) repetido refaz o trabalho.**
   `sel_cone.entry(nome.to_string())` ignora o retorno do `insert`
   (`lib.rs:731`): o mesmo `x.foo` em mil lugares reprocessa mil vezes os
   pendentes de `foo`.
3. **O laço externo recomeça do zero.** Cada rodada de
   `proprio.rs::ponto_fixo` (`proprio.rs:185-228`) recalcula o mundo
   inteiro, emite tudo e confere o texto todo. Seis rodadas são seis mundos.
   O verificador ainda chama `Mundo::alcancada` (`lib.rs:159-169`), sem
   memo e com uma varredura de todas as classes por consulta
   (`verificar.rs:481-515`).

### 2.1 O algoritmo

A semântica não muda. A regra continua sendo a de `alcanca`: o membro do
dono `M` atende o receptor de cone `T` se `T ⊑ M` (herdado), se `M ⊑ T`
(sobrescrito) ou se **existe classe instanciada `S` com `S ⊑ T` e `S ⊑ M`**.
Muda o cálculo:

1. **Supertipos pré-calculados.** `sup[C]` é o fecho transitivo
   (superclasse, mixins, interfaces, `on`), feito uma vez por classe como
   vetor de bits. `e_subtipo` passa a ser O(1). Substitui
   `ancestrais`/`contem_na_cadeia`.
2. **Subtipos instanciados por cone.** Ao instanciar `S`, para cada
   `T ∈ sup[S]`, `S` entra em `inst_sub[T]`. O terceiro caso da regra
   passa a ser `inst_sub[T].any(|S| M ∈ sup[S])`, que olha só os subtipos
   instanciados **de `T`**.
3. **Par (nome, cone) uma vez.** `cones[nome]` é um conjunto, e um
   `insert` que devolve `false` encerra o uso (o `addReceiverConstraint`
   do dart2js).
4. **Pendentes por (nome, dono).** Assim cada direção acorda só o que pode
   acordar:
   * *seletor novo `(n, T)`*: percorre os pendentes de `n` uma vez;
   * *classe nova `S`*: para cada `T ∈ sup[S]` com nomes registrados e cada
     nome `n` desses, acorda os pendentes de `n` cujo dono está em `sup[S]`.
     É o `_processInstantiatedClass` → `processClassMembers`
     (`resolution/enqueuer.dart:135-139`), invertido para os pendentes
     restritos.
5. **Chaves por `SymbolId`**, não `String`: a chave de escrita `foo_=` já
   existe no `instance_members` (`elements/src/model.rs:311`).
6. **O `Mundo` final guarda `sup` e `inst_sub`.** `seletor_vivo_para` e o
   verificador consultam com o mesmo custo.
7. **O laço externo reaproveita o motor.** As rodadas de uma fase só
   acrescentam raízes (`proprio.rs:221-227`), então o `Motor` fica vivo
   (`semear` + `rodar`), e o `Mundo` de cada rodada é uma fotografia dele.

Meta: o mundo do `new_sali` em segundos por rodada. A seção 5 diz como
medir.

---

## 3. Tamanho: o que entra, em ordem de ganho medido

Cada passo é medido no `new_sali` e no `limitless_ui`, e só fica ligado se
diminuir o arquivo sem quebrar a validação (§5).

### 3.1 Nomes de propriedade: minificar os membros Dart

O que o dart2js faz: um nome curto por seletor, dado por frequência
(`js_backend/namer.dart:197-206` e `:416-453`, `minify_namer.dart:8-117`,
`frequency_namer.dart:72-86`). No nosso arquivo os nomes são 25% do
total.

Como fazemos: o renomeio de propriedades do `oxc_minifier`
(`oxc/crates/oxc_minifier/src/property_mangler.rs`), na mesma pilha que
já usamos (`oxc_parser` → `oxc_mangler` → `oxc_codegen`). Ele aceita:

* `reserved`, os nomes que não mudam (`:107-120`);
* `cache`, um mapa autoritativo nome → nome curto, ou `None` para manter
  (`:21-68`);
* strings anotadas com `/* @__KEY__ */`, renomeadas junto com a
  propriedade (`:267-273`, `:504-520`).

As strings soltas (`dsend(o, "foo")`) não são renomeadas, mas o nome fica
"ocupado" (`:271-280`). O swc fica de fora: não aceita mapa de entrada e
renomearia a propriedade sem renomear a string do `dsend`
(`swc_ecma_minifier/src/pass/mangle_props.rs:130-137`).

O conjunto **renomeável** é calculado por nós a partir do modelo, nunca do
texto: os nomes de membros Dart (de instância e estáticos, mais os de topo
das bibliotecas) de classes que **não** são nativas nem de interop. Desse
conjunto saem:

* todo nome que aparece num *template* `JS()` vivo, porque ali é
  propriedade JS;
* nomes `external`, `@JS`/`@JSName` e de tipos de extensão de interop;
* nomes de parâmetros nomeados e de campos de *record* (o `Function.apply`
  e o `dart.dcall` montam a chave em execução);
* toda *string* literal do arquivo com a forma de identificador. Isso
  cobre `dsend`/`dload`/`dput`/`bind`, `js_util.getProperty` e as
  mensagens de `NoSuchMethodError`. O nome que chega à execução continua o
  de origem, e a saída do corpus não muda;
* o protocolo do JS: `toString`, `valueOf`, `toJSON`, `then`, `length`,
  `name`, `message`, `stack`, `constructor`, `prototype`, `call`, `apply`,
  `bind`, `next`, `done`, `value`, `return`, `throw`, `get`, `set`,
  `handleEvent`.

O mapa nome → nome curto é calculado por frequência sobre o texto
(propriedade mais usada, nome mais curto) e passado inteiro pelo `cache`
do `oxc`. O resultado é determinístico.

### 3.2 Receitas rti e nomes de biblioteca curtos

As receitas, o JSON das regras e os `setLibraryUri`/`privateName` repetem
o identificador inteiro da biblioteca, e somam 20% do arquivo. O runtime só
usa o que vem depois do `|`, no `_rtiToString`. A parte da biblioteca é
opaca, então pode virar um identificador curto (índice em base 36) em toda
receita, regra e `addRtiResources`. O mesmo mapa vale para o
`dart.privateName(lib, "x")`.

`setLibraryUri` continua (a mensagem de `TypeError` com dois tipos de
mesmo nome lê a URI, `errors.dart:113`), mas só nas classes cujo nome
simples se repete no programa.

### 3.3 Globais nunca escritas e código morto por instrução

O que o dart2js faz: campo estático ou de topo que nenhum código vivo
escreve vale o inicializador, e o desvio morto sai
(`inferrer/engine.dart:850-884`, `ssa/` em `-O4`). Nós fazemos assim:

1. o mundo registra **escritas** de variáveis de topo e estáticas, além das
   leituras (o `alvo_de_escrita` já distingue `foo_=` em membros);
2. uma variável viva, não `late`, com inicializador literal `bool`, `int`,
   `null` ou *string* curta, e sem escrita viva, é dobrada no uso como as
   constantes (`sdk_proprio::constante_inline`);
3. a compressão do `oxc_minifier` (`Minifier::minify`, `lib.rs:132`, com
   `CompressOptions::smallest()`, `options.rs:66`) apaga o
   `if (false) {…}`, dobra constantes e remove declarações locais sem uso.
   `treeshake.manual_pure_functions` recebe `dart.fn`, `dart.fnType` e
   `dart.privateName` (`options.rs:173`).

O `Inspector` e o resto do devtools do ngdart ficam sem uso e saem do
mundo.

### 3.4 Formas compactas

* um campo `late` sem inicializador vira um acessor gerado por uma função
  auxiliar do runtime, em vez do par `get`/`set` escrito por extenso;
* `dart.privateName(L, "x")` é agrupado por biblioteca numa chamada só;
* o `$constCache`/`$C` de cada biblioteca passa a ser um auxiliar único.

### 3.5 Fora deste plano

* Carregamento diferido: nenhum dos projetos usa `deferred`.
* Inlining do SSA do dart2js (`ssa/builder.dart:7851-8075`).

---

## 4. Ordem

| # | passo | ganho esperado no `new_sali` |
| --- | --- | --- |
| 1 | mundo incremental (§2.1) | tempo: de 707 s para segundos |
| 2 | nomes de propriedade (§3.1) | −4 a −5 MB |
| 3 | bibliotecas curtas nas receitas (§3.2) | −3 MB |
| 4 | globais nunca escritas + compressão do `oxc` (§3.3) | −2 a −4 MB |
| 5 | formas compactas (§3.4) | −1 MB |

Cada passo é medido de novo antes do seguinte. A estimativa só ordena o
trabalho, não substitui a medida.

## 5. Validação de cada passo

1. `dartforge-diferencial --producao`: corpus/js 238/238 iguais à VM.
2. Testes de `mundo`, `emit_js`, `emit_js_producao` e `elements`.
3. `limitless_ui/example`: e2e 26/26.
4. `new_sali/frontend`: `scripts/fluxo.mjs`, os 11 passos sem erro.
5. Tamanho (bruto e gzip) e tempo por fase, na tabela da §1.1.

---

## 6. Resultado parcial (2026-10-01)

O que entrou:

* **Mundo incremental** (§2.1): `Hierarquia`, `inst_sub`, pendentes por
  (nome, dono), par (nome, cone) uma vez só, e o motor retomável entre as
  rodadas (`dartforge_mundo::Calculo`).
* **Renomeio de propriedades** (§3.1). Fica em
  `emit_js_producao/src/propriedades.rs` (o conjunto) e `minificar.rs` (o
  `PropertyMangler` do `oxc_minifier`, em duas coletas: os candidatos que o
  `oxc` enxerga menos os livres vão para os reservados). As palavras dos
  *templates* reservadas são só as que podem ser propriedade: depois de
  `.`, antes de `:` ou entre aspas.
* **Receitas rti com a biblioteca curta** (§3.2, `emit_js::tag_de_receita`).
* **`assert` desligados por padrão e condições constantes** (§3.3,
  `dartforge_mundo::Constantes`). `--enable-asserts` os liga, e o
  `dartforge-diferencial` passa a flag, porque o oráculo roda a VM com
  `--enable-asserts`.
* **Compressão do `oxc_minifier`** antes do renomeio. `dart.fn` **não** é
  função pura: ela devolve a *closure*, e marcá-la pura fazia o `oxc`
  supor que o argumento nunca é chamado (`94_null_safety_promocao`,
  `83_completer`).
* **Formas compactas** (§3.4):
  * `dart.lateField` para os `late` privados sem inicializador;
  * deduplicação de *strings* (`var t$S…` no topo da IIFE);
  * campo público sem sobrescrita como propriedade comum
    (`sdk_proprio::campo_nao_virtual`). Ficam acessores os campos do SDK, os
    de mixins e os de nome de membro de supertipo nativo: estes são lidos
    pela chave `dartx` que o `defineExtensionAccessors` copia do *getter*,
    e era isso que quebrava o `ArgumentError.name` do SDK.

| projeto | antes, bruto | gzip | agora, bruto | gzip | dart2js -O4 | gzip |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `new_sali/frontend` | 24.895.868 | 3.578.704 | **12.719.130** | **2.743.682** | 7.616.039 | 2.025.116 |
| `limitless_ui/example` | 14.043.321 | 2.074.627 | **7.036.699** | **1.579.793** | 4.511.811 | 1.195.800 |
| `01_print` | 257.738 | 46.988 | **176.102** | **40.664** | 34.929 | 11.551 |
| `120_convert_json` | 408.091 | 75.468 | **284.323** | **66.619** | 58.553 | 18.965 |
| `135_convert_json_classes` | 522.234 | 100.292 | **370.735** | **88.785** | 101.196 | 32.005 |

| projeto | antes | agora | dart2js |
| --- | ---: | ---: | ---: |
| `new_sali/frontend`, compilação inteira | 14 min 22 s | **35–45 s** | 57 s |
| — só o mundo fechado (6 rodadas) | 707 s | **0,8–1,2 s** | — |
| `limitless_ui/example`, compilação inteira | 8 min 23 s | **13 s** | — |

Validação deste estado:

* corpus/js `--producao` 238/238;
* testes dos crates verdes;
* `limitless_ui` 26/26 no e2e;
* `new_sali` com o `fluxo.mjs` igual ao do bundle do `dart2js`: 11
  passos, 0 com erro. Os passos 2 e 4 acusam botão ausente nos dois, porque
  a página abre já autenticada.

O tempo já está na ordem do dart2js. O tamanho ainda não: falta 1,7× no
`new_sali` (bruto) e 1,35× em gzip. Os próximos passos medidos estão na
§7.

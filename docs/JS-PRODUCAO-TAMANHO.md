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

| projeto | nosso | dart2js -O4 | nosso, tempo | dart2js, tempo |
| --- | ---: | ---: | ---: | ---: |
| `new_sali/frontend` | 24.895.868 | 7.616.039 | 14 min 22 s | 57 s |
| `limitless_ui/example` | 14.043.321 | 4.511.811 ¹ | 8 min 23 s | — |
| `01_print` | 257.738 | 34.929 | 0,3 s | 2,6 s |

¹ O build oficial (`build_web_compilers --release`).

Tempo do nosso no `new_sali`, por fase (`dartforge-jsprod`, soma das
rodadas do ponto fixo, 6 rodadas):

| fase | tempo |
| --- | ---: |
| mundo fechado (`dartforge_world::calcular_com`) | **707 s** |
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

A fila do `Motor` (`crates/world/src/lib.rs:1256-1270`) já processa cada
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

1. `dartforge-differential --production`: corpus/js 238/238 iguais à VM.
2. Testes de `mundo`, `emit_js`, `emit_js_production` e `elements`.
3. `limitless_ui/example`: e2e 26/26.
4. `new_sali/frontend`: `scripts/flow.mjs`, os 11 passos sem erro.
5. Tamanho (bruto) e tempo por fase, na tabela da §1.1.

---

## 6. Resultado parcial (2026-10-01)

O que entrou:

* **Mundo incremental** (§2.1): `Hierarquia`, `inst_sub`, pendentes por
  (nome, dono), par (nome, cone) uma vez só, e o motor retomável entre as
  rodadas (`dartforge_world::Calculo`).
* **Renomeio de propriedades** (§3.1). Fica em
  `emit_js_production/src/propriedades.rs` (o conjunto) e `minificar.rs` (o
  `PropertyMangler` do `oxc_minifier`, em duas coletas: os candidatos que o
  `oxc` enxerga menos os livres vão para os reservados). As palavras dos
  *templates* reservadas são só as que podem ser propriedade: depois de
  `.`, antes de `:` ou entre aspas.
* **Receitas rti com a biblioteca curta** (§3.2, `emit_js::tag_de_receita`).
* **`assert` desligados por padrão e condições constantes** (§3.3,
  `dartforge_world::Constantes`). `--enable-asserts` os liga, e o
  `dartforge-differential` passa a flag, porque o oráculo roda a VM com
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

| projeto | antes, bruto | agora, bruto | dart2js -O4 |
| --- | ---: | ---: | ---: |
| `new_sali/frontend` | 24.895.868 | **12.719.130** | 7.616.039 |
| `limitless_ui/example` | 14.043.321 | **7.036.699** | 4.511.811 |
| `01_print` | 257.738 | **176.102** | 34.929 |
| `120_convert_json` | 408.091 | **284.323** | 58.553 |
| `135_convert_json_classes` | 522.234 | **370.735** | 101.196 |

| projeto | antes | agora | dart2js |
| --- | ---: | ---: | ---: |
| `new_sali/frontend`, compilação inteira | 14 min 22 s | **35–45 s** | 57 s |
| — só o mundo fechado (6 rodadas) | 707 s | **0,8–1,2 s** | — |
| `limitless_ui/example`, compilação inteira | 8 min 23 s | **13 s** | — |

Validação deste estado:

* corpus/js `--production` 238/238;
* testes dos crates verdes;
* `limitless_ui` 26/26 no e2e;
* `new_sali` com o `flow.mjs` igual ao do bundle do `dart2js`: 11
  passos, 0 com erro. Os passos 2 e 4 acusam botão ausente nos dois, porque
  a página abre já autenticada.

O tempo já está na ordem do dart2js. O tamanho ainda não: falta 1,7× no
`new_sali`. Os próximos passos medidos estão na
§7.

### 6.1 Segunda rodada

Entraram mais quatro mudanças:

* **Nomes privados do programa como propriedades** (`_x$1c`, com o rótulo
  da biblioteca) em vez de `Symbol` (`dart.privateName`). A privacidade do
  Dart é por biblioteca, e o rótulo já a separa. Assim saem uma declaração
  por nome e o colchete em cada acesso, e o renomeio de propriedades
  encurta o nome. As bibliotecas do SDK continuam com símbolos. Os acessos
  `x["_n$1c"]` contam para o verificador e para os seletores dinâmicos
  (`dartforge_emit_js::nome_dart_de_privado`).
* **`dart.lateFields`**: uma chamada por classe, com as chaves num objeto
  literal. O auxiliar cria os símbolos de armazenamento, e o construtor não
  escreve mais `= null` nesses campos.
* Assinaturas com `__proto__` no objeto literal, que é a forma do DDC.
* Assinaturas estáticas (`setStatic*Signature`) só com os nomes do despacho
  dinâmico. `setLibraryUri` só nas classes cujo nome simples se repete,
  que são as únicas em que `_castErrorMessage` o lê (`errors.dart:110-117`).

A reserva de *strings* passa a ser lida **antes** e **depois** da
compressão. A compressão junta listas de *strings* (`["a","b"]` vira
`"a.b".split(".")`), e com isso o `defineExtensionAccessors` do `dart:html`
perdia os nomes. O e2e do `limitless_ui` pegou isso e o corpus não, porque
não tem DOM.

| projeto | rodada 1 | agora | dart2js -O4 |
| --- | ---: | ---: | ---: |
| `new_sali/frontend` | 12.719.130 | **12.752.196** | 7.616.039 |
| `limitless_ui/example` | 7.036.699 | **7.248.222** | 4.511.811 |
| `01_print` | 176.102 | **171.534** | 34.929 |

Em bruto, o ganho das formas compactas foi comido pelos nomes que voltaram
a ser reservados quando a reserva passou a ser lida antes da compressão.

Validação:

* corpus 238/238;
* testes verdes;
* `limitless_ui` 26/26;
* `new_sali` com o `flow.mjs` igual ao controle `dart2js`.

O `new_sali` compila em 29–45 s, contra 57 s do `dart2js`.

## 7. O que falta para o tamanho do dart2js

Medido no `new_sali` (12,75 MB): pontuação, palavras-chave e
identificadores são ~55% (o volume de código no contrato do DDC), os nomes
de propriedade ~22% e as *strings* ~25%. Destas, as tabelas de dados dos
pacotes (bidi do `pdf_plus`, entidades HTML, linguagens do `highlight`) o
`dart2js` também tem, e o JSON das regras rti é 0,7 MB. O que o `dart2js`
faz e nós não:

1. **Nomes de parâmetros nomeados.** Ficam reservados porque a checagem
   do `dcall` compara as chaves do objeto de argumentos com os nomes que
   estão dentro das receitas rti (*strings*), e o `Function.apply` monta a
   chave pelo `Symbol`. Renomeá-los exige trocar a convenção de chamada
   nomeada (o dart2js usa *stubs* `$named`, `codegen_world_builder.dart:674-733`).
2. **Checagens implícitas** (`omitImplicitChecks` do `-O4`,
   `options.dart:1102-1126`). São ~7.400 `[_as]` no `new_sali`. O contrato
   do DDC as exige, e o corpus compara com a VM, que faz as checagens.
3. **Regras rti só para as classes testadas**
   (`RuntimeTypesChecks.requiredClasses`, `runtime_types.dart:26-45`).
4. ***Inlining*** e devirtualização do SSA (`ssa/builder.dart:7851-8075`):
   `unsafeCast`, *getters* triviais e `dart.fn` de *closures* que não
   precisam de assinatura (`methodNeedsSignature`,
   `runtime_types_resolution.dart:942`).

Cada um desses é uma mudança de contrato do emissor, não uma compactação
de texto: é especificação nova, com o mesmo método.

## 8. Mudanças de contrato (decisão do dono: tamanho do dart2js)

A ordem e as regras foram decididas pelo dono:

* cada mudança tem a sua spec curta e é validada com o corpus `--production`,
  o e2e do `limitless_ui` e o `new_sali`;
* o padrão mantém a semântica da VM, como o `dart2js -O2`;
* o corte das checagens implícitas no estilo `-O4` só entra como opção
  explícita, documentada como insegura;
* as medidas são contra o `-O2` (padrão) e contra o `-O4` (com a opção).

### 8.1 Regras rti e marcas de interface só para os tipos testados

O que o `dart2js` faz: emite as regras de subtipagem só das classes cujas
checagens são alcançáveis (`RuntimeTypesChecks.requiredChecks` e
`requiredClasses`, `js_backend/runtime_types.dart:26-45`).

No runtime compartilhado (`dart:_rti`), `S <: T` com `T` de interface
consulta só `regra[S][T]` (`_isSubtype`, `lookupSupertype`), e o teste
rápido de `is` lê a marca `$is_<T>` que o `addRtiResources` pôs no
protótipo. `T` é sempre um tipo que alguma receita do arquivo cita: o alvo
de um `is`/`as`/`catch`, um argumento de tipo, uma substituição. Então
(`emit_js_production/src/regras.rs`):

* **citados** são os nomes `lib|Classe` do arquivo inteiro, menos as chaves
  das regras e as listas do `addRtiResources`, mais os dos **valores** das
  regras (argumentos de supertipo e substituições, que viram alvo quando
  avaliados);
* uma entrada de supertipo (`"lib|T": [...]`) e uma marca de interface (os
  itens 2… da lista do `addRtiResources`) **saem** se `T` não é citado. As
  substituições `"T.E"` e as regras de encaminhamento ficam todas;
* regra que fica vazia sai também: sem ela, o `_isSubtype` dá o mesmo
  `false` (regra ausente).

`DARTFORGE_JSPROD_RULES=0` desliga.

| projeto | antes | depois |
| --- | ---: | ---: |
| `new_sali/frontend` | 12.752.196 | **12.601.153** |
| `limitless_ui/example` | 7.248.222 | **7.073.375** |
| `01_print` | 171.534 | **157.690** |

Validação: corpus 238/238, `limitless_ui` 26/26, `new_sali` igual ao
controle.

### 8.2 Parâmetros nomeados com chave curta

O que o `dart2js` faz: os nomes dos argumentos nomeados viram parte do
nome minificado do *stub* de chamada (`$named`,
`universe/codegen_world_builder.dart:674-733`). No contrato do DDC o nome
é a chave de um objeto (`f(1, {limite: 2})`), lida no prólogo (`opts &&
"limite" in opts ? opts.limite : …`), e também está na receita rti do tipo
da função (`~(core|int{limite:core|int})`). A checagem da chamada dinâmica
(`_checkAndCall`) compara essa receita com as chaves do objeto. Como a
receita é uma *string*, o nome não podia passar pelo renomeio de
propriedades e ficava reservado, e junto com ele todo membro homônimo.

A regra (`emit_js_production/src/nomeados.rs`, `Ctx::nomeado`):

* um mapa **global por nome**, nome → `$` + base 36, aplicado ao mesmo
  tempo nas chaves da chamada (`call.rs`), no prólogo (`body.rs`,
  `module.rs`), no repasse `super` e no redirecionamento de tipo de
  extensão, e na receita rti do tipo de função. Por ser função só do nome,
  chamador e chamado concordam sem se conhecer, inclusive nas chamadas
  dinâmicas;
* na receita, os nomeados são ordenados pela **chave curta**, porque o rti
  compara as listas em ordem de nome;
* ficam com o nome de origem os nomeados de `external` e de classes de
  interop (são chaves de objeto do JavaScript) e as palavras dos
  *templates* `JS()` vivos;
* com um `noSuchMethod` próprio vivo, o mapa é desligado: a `Invocation`
  de uma chamada dinâmica mostraria as chaves;
* com `Function.apply` vivo, que monta as chaves a partir do `Symbol` do
  nome, o mapa vai também para o runtime. O `#[#] = #` do `core_patch`
  passa pela tradução `dart.nomeadoJS` (`sdk_proprio::emitir_intrinseco`).
  O `new_sali` usa `Function.apply`, pelo `uuid` e pelo `petitparser`.

Efeito visível: o `toString` de um tipo de função com nomeados mostra a
chave curta. Nenhum programa do corpus imprime um.
`DARTFORGE_JSPROD_NAMED=0` desliga.

| projeto | antes | depois |
| --- | ---: | ---: |
| `new_sali/frontend` | 12.601.153 | **12.356.255** |
| `limitless_ui/example` | 7.073.375 | **6.942.798** |
| `01_print` | 157.690 | **156.012** |

Validação: corpus 238/238 (entre eles os quatro com `Function.apply`),
`limitless_ui` 26/26, `new_sali` igual ao controle.

### 8.3 `--omit-checks` (o `-O4` do dart2js), opcional e inseguro

O padrão continua com a semântica da VM, como o `dart2js -O2`. Com
`--omit-checks` (ou `-O4`) saem as conversões implícitas de `dynamic`
(`FnEmitter::coerce_to`) e as checagens de covariância de parâmetro
(`sdk_proprio::verificacoes_de_covariancia`), como o `omitImplicitChecks`
do `dart2js` (`options.dart:1102-1126`). Um programa com erro de tipo
segue adiante em vez de lançar `TypeError`. A comparação justa com o
`dart2js -O4` é com a opção ligada.

Medido no `new_sali`:

| configuração | bytes |
| --- | ---: |
| padrão (semântica da VM) | 12.356.255 |
| `--omit-checks` | 12.286.128 |
| `dart2js -O2` | 8.022.592 |
| `dart2js -O4` | 7.616.039 |

A opção tira só 70 KB. No `dart2js`, a distância entre `-O2` e `-O4`
(406 KB) vem também de `--trust-primitives` e do *inlining* que o
`-O4` libera.

### 8.4 `this` repetido como local

O que o `dart2js` faz: o receptor de um método que o cita muito vai para um
local de uma letra (a variável do receptor em `ssa/codegen.dart`), e cada
citação custa um byte em vez dos quatro de `this`, que nenhum minificador
encurta.

A regra (`emit_js_production/src/aliasthis.rs`, antes da minificação): numa
função (não flecha) cujo corpo cita `this` `k` vezes, com `3k > 11` (cada
citação economiza 3 bytes, contra os ~11 de `let t=this;` depois da troca de
nomes), o corpo ganha `let t$this = this;` no começo, e cada `this` dele vira
`t$this`, inclusive dentro das flechas, que herdam o mesmo `this`. Ficam como
estão o `constructor` de classe (antes do `super()` o `this` não existe), os
corpos com diretivas, o `this` dos valores padrão de parâmetro (roda antes da
declaração) e o dos inicializadores de campo de classe, que não é o da função
envolvente. `DARTFORGE_JSPROD_THIS=0` desliga.

O limiar foi medido: com `2k > 13` o `new_sali` sai com 12.103.052 bytes; com
`3k > 11`, 12.082.759.

| projeto | antes | depois |
| --- | ---: | ---: |
| `new_sali/frontend` | 12.361.194 | **12.082.759** |
| `limitless_ui/example` | 6.946.316 | **6.693.718** |
| `01_print` | 156.012 | **155.403** |

Validação (2026-10-06): corpus `--production` 238/238, `limitless_ui` 26/26 no
e2e, `new_sali` com o `flow.mjs` nos 11 passos sem erro, igual ao controle
sem o passo.

## 9. Rodada de 2026-10-12 (macOS): rumo a menos que o `dart2js -O4`

Meta do dono: arquivo **menor** que o do `dart compile js -O4`, com o corpus
de produção 238/238 igual à VM. Medido antes de mexer
(`scripts/measure-js-production-tokens.py`, categorias de token do arquivo
minificado; o `dart2js` é o `build_runner build --release` com `-O4`):

| categoria | `new_sali` nosso | `new_sali` dart2js |
| --- | ---: | ---: |
| nomes de propriedade (`.x`) | 2.985.036 | 816.930 |
| chaves de objeto (`x:`) | 319.424 | 65.883 |
| *strings* e *templates* | 3.096.331 | 2.120.568 |
| pontuação | 2.743.261 | 2.085.301 |
| identificadores | 1.483.127 | 1.162.202 |
| palavras-chave | 962.114 | 677.675 |
| total | 12.077.217 | 7.614.136 |

A maior diferença eram os nomes de propriedade: 1,72 MB em nomes de nove
letras ou mais que o renomeio deixava como estavam.

### 9.1 Só as *strings* em posição de nome reservam

A regra da §3.1 reservava **toda** *string* com forma de identificador do
arquivo. No `new_sali` isso prendia os campos dos modelos (`"codProcesso"`
como chave de JSON ou nome de coluna), os nomes das constantes de `enum`
(`"otherNeutrals"`, o `_name`) e as mensagens dos `LateError`. Agora
(`minificar::StringsIdentificador`):

* as *strings* do runtime (o *bootstrap* e o módulo do SDK, antes da marca
  `"dartforge:sdk-end"` que `proprio::montar` põe depois do SDK)
  continuam todas reservadas;
* no programa, só as que estão nos argumentos de uma chamada `dart.X` (fora
  das que recebem mensagens, constantes e descrições: `privateName`,
  `const*`, `nullCheck`, `throw`, `fn`, `str`, `lateFields`,
  `recordLiteral`…) ou `js_util.X`, à esquerda de `in`, e nos argumentos de
  `defineProperty`/`hasOwnProperty`/`Reflect.get`… O contexto vale até a
  próxima chamada, `new` ou corpo de função com instruções (os *getters* do
  `defineLazy` não herdam; a flecha `() => ["root"]` das assinaturas
  estáticas herda).

`DARTFORGE_JSPROD_STRINGS=all` volta à regra antiga, e
`DARTFORGE_JSPROD_PROPS_WHY=a,b` diz por que cada nome fica.

### 9.2 Membros de biblioteca como variáveis

`dart.dsend(…)`, `core.print(…)`, `L$pacote__lib.Classe.new(…)`: no contrato do
DDC a biblioteca é um objeto e o membro de topo, uma propriedade. Eram 110 mil
acessos no `new_sali`, e o nome do membro disputava o espaço dos nomes de
propriedade: `dart.throw`, `dart.bind`, `dart.const`, `html.document`, o
`setAttribute` e o `appendText` do ngdart ficavam por extenso porque o mesmo
nome é propriedade do JavaScript ou do DOM. `achatar.rs` troca `N.x` por uma
variável `N$$x` declarada no topo do arquivo, que o `oxc` encurta como a um
local, quando:

* `N` é declarado no topo com `Object.create(…)` e não é sombreado;
* `N` só é usado como objeto de `N.x`/`N["x"]` ou como primeiro argumento de
  `defineLazy`/`copyProperties` (cujas chaves ficam propriedades) e de
  `privateName`;
* `x` tem uma escrita `N.x = …`, não é *getter* de `defineLazy`, e nenhuma
  função atribuída a ele lê `this` (nem há `delete N.x`).

`DARTFORGE_JSPROD_FLATTEN=0` desliga.

### 9.3 Receita com o nome simples

A classe cujo nome simples nenhuma outra do programa (SDK incluído) usa é
escrita nas receitas e nas regras rti sem a biblioteca: `|Padding`, `|int`
(`Ctx::class_recipe`). O `|` inicial é começo de identificador para o
*parser* de receitas (`Recipe.isIdentifierStart`), o `_rtiToString` corta até
ele (mostra `Padding`, como antes), e ele marca a receita para o verificador
(que acha a classe pelo nome único) e para a poda das regras. Nomes repetidos
continuam `rótulo|Nome`. Os nomes que o emissor escrevia à mão
(`core|List` e `async|Future` do `JS_GET_NAME`, `core|Object*`,
`async|StreamIterator`, `_interceptors|LegacyJavaScriptObject`) passam por
`Ctx::receita_de`. `DARTFORGE_JSPROD_SHORT_RECIPE=0` desliga.

### 9.4 *Strings* de dados com forma de identificador também deduplicadas

Com a reserva da §9.1, as *strings* com forma de identificador fora de
posição de nome são dados, e entram na deduplicação da §3.4 (`"button"`,
`"label"`, `"NgValueAccessor"`…). Ficam literais as do runtime, as de
posição de nome, as chaves de `x["…"]` e de `in`. A reserva ignora as
declarações `t$S…` da deduplicação.

### 9.5 Id curto por classe nas receitas, e a tabela de nomes

A §9.3 virou um passo intermediário: no modo SDK toda classe é `|id`
(`dartforge_emit_js::ids_de_receita`: as classes mais comuns do SDK primeiro,
com uma letra, depois as outras na ordem do programa, `a`…`$`, `aa`…). É função
só do programa, então o emissor, o verificador (que acha a classe pelo id) e a
montagem chegam ao mesmo id.

O nome que o `_rtiToString` mostra sai de uma tabela `t$N` (`"|id": "Nome"`)
que `proprio::montar` põe no arquivo, ligada ao
`unmangleGlobalNameIfPreservedAnyways` do `dart:_js_names` do DDC (que
devolvia `null`): o `_unminifyOrTag` já consulta essa função antes de tudo.
Entram na tabela só as classes que uma mensagem pode mostrar
(`regras::mostraveis`): as citadas fora das chaves das regras e fora das
listas do `addRtiResources`, mais a primeira de cada lista. Uma classe que só
aparece como supertipo nas regras nunca vira objeto rti.
`DARTFORGE_JSPROD_SHORT_RECIPE=0` volta à receita `rótulo|Nome`.

A tabela custava 130 KB no `new_sali` como objeto JSON. Agora é uma
*string* só, com os nomes separados por espaço na ordem dos ids (vazio o que
não aparece) e a função `t$I` que converte o id no número
(`dartforge_emit_js::numero_de_id`): um byte por classe em vez da chave e das
aspas, −35 KB.

### 9.6 Construtores (`C.new`, `C.nome`) renomeados

`new C.new(…)` e `(C.new = function(…){…}).prototype = C.prototype` custavam
`.new` por extenso em 30 mil lugares do `new_sali`: o nome ficava reservado
porque as fábricas saem como `static ["new"](…)` (chave entre aspas no SDK) e
porque os *templates* `JS()` do runtime dos *records* citam `#.new`. Agora:

* os nomes JS dos construtores (`new` e os nomeados) entram no conjunto
  renomeável (`propriedades.rs`), fora das classes de interop;
* `new` nos *templates* é o construtor Dart (nenhuma propriedade do
  JavaScript se chama assim), e não reserva;
* na primeira visita da reserva (antes da compressão), as chaves literais
  (`static ["new"]()`, `x["foo"]`, `{"a": 1}`) não reservam: a compressão as
  troca pela forma sem aspas, que o renomeio trata como as outras. A que
  sobrar entre aspas depois dela reserva, na segunda visita.

O `classGetConstructorType` do DDC lê `'new'` por *string*; se ele estiver
vivo, a *string* do runtime reserva o nome e nada muda.

### 9.7 O nome do `dart.bind` renomeado junto

O *tearoff* `dart.bind(o, "m")` lê o método (`o[m]`) e a assinatura
(`sig[m]`) pelo nome da propriedade, e as duas já são renomeadas. Então a
*string* do `bind` (e do `bindCall`) do programa não reserva o nome: depois do
renomeio, `minificar::reescrever_ligacoes` troca o valor dela pelo nome novo.
No `new_sali` isso solta os `_handleEvent_N` dos *templates* do ngdart, que
ficavam por extenso. A deduplicação continua sem tocar nessas *strings*.

### 9.8 Formas compactas: o protótipo do construtor

`(C.nome = function(…) {…}).prototype = C.prototype;`, uma instrução por
construtor Dart, vira `t$P(C.nome = function(…) {…}, C);`, com
`t$P = function(f, c) { f.prototype = c.prototype; }` declarada uma vez
(`formas.rs`, depois do achatamento). A atribuição acontece antes da chamada,
como antes. `DARTFORGE_JSPROD_SHAPES=0` desliga.

### 9.9 Receitas fechadas do programa avaliadas uma vez

`t$R("receita")` (o `_Universe.eval` de uma receita fechada) aparecia 73 mil
vezes no `new_sali`. No programa (depois da marca do fim do SDK), cada receita
usada duas vezes ou mais vira uma variável `t$T…`, declarada logo depois do
SDK: `_Universe.eval` devolve o objeto canônico do universo, então avaliar
uma vez dá o mesmo objeto, e cada uso fica uma variável em vez de uma
chamada. O SDK fica como está: código dele roda durante a carga, antes da
declaração. Também em `formas.rs`; `DARTFORGE_JSPROD_FIXED_RECIPES=0`
desliga. A troca do protótipo (§9.8) passou a ser duas trocas pequenas, para
compor com as de dentro da função.

### 9.10 Prólogo dos parâmetros nomeados e chaves por frequência

O prólogo `o && "k" in o ? o.k : d` (6 mil no `new_sali`) vira `t$O(o, "k",
d)` quando `d` é literal ou variável (`formas.rs`): o valor padrão Dart é
constante, então avaliá-lo antes não muda nada. `t$O` conta como chamada de
nomes para a reserva (a chave é lida por *string*). As chaves curtas dos
nomeados (§8.2) passaram de base 36 por ordem alfabética para a numeração dos
ids de receita (uma letra entre 54, depois 64), os nomes mais declarados
primeiro.

## 10. Rodada de 2026-10-13 (macOS): o piso do runtime

Medido antes de mexer, no corpus inteiro (237 programas que compilam;
`dart compile js -O4` do SDK 3.6.2 de macOS), num `01_print` de 128.592 bytes
(35.923 com gzip) contra 34.926 do `dart2js`. Num programa pequeno quase tudo
é runtime, então esta rodada atacou o que o runtime carrega sem precisar. Cada
passo foi medido sozinho, e o corpus `--production` deu 238/238 depois de cada um.

### 10.1 Substituições de variável de tipo só as citadas

A regra de cada classe trazia `"Classe.T": "1"` para os parâmetros de **todos**
os supertipos (`"EfficientLengthIterable.T"`, `"HideEfficientLengthIterable.T"`,
`"Iterable.E"`…): 7,5 KB dos 12,5 KB do JSON das regras no `01_print`. O
`_Universe.evalTypeVariable` só procura o nome que o *parser* de receitas leu
de uma receita do arquivo (`|j<Iterable.E>`), e os valores das regras nunca
citam variável pelo nome (usam os índices `"1"`, `"2"`). `rti_rules::prune`
passou a tirar a entrada cujo nome nenhum texto fora do JSON escreve.
`DARTFORGE_JSPROD_TYPE_VARIABLES=0` desliga. `01_print`: 128.592 → 121.993.

### 10.2 Nativos fora dos *built-ins* esperam código estrangeiro

Uma classe nativa viva como tipo (ou por uma interface que ela implementa,
como `JavaScriptIndexingBehavior`) era instanciada: todo programa levava as
visões de `dart:typed_data` com os `registerExtension`, as regras e as marcas.
Um `Uint8Array` só existe no programa se alguma função de `dart:typed_data`,
`dart:_native_typed_data`, da família `dart:html` ou das bibliotecas de
interop estiver viva, ou uma função `external` fora do SDK
(`dartforge_world::FOREIGN_LIBRARIES`): é o `NativeBehavior.typesInstantiated`
do `dart2js`, grosso ao nível da biblioteca. Até lá os nativos ficam retidos e
são instanciados quando a porta abre. Os *built-ins* (`dart:_interceptors`)
não mudam. `DARTFORGE_JSPROD_NATIVE_GATE=0` desliga. `01_print`: → 111.018.

### 10.3 Funções triviais *inline*

`inline.rs`, depois do achatamento: um método estático de classe
(`C = class …`) ou uma função atribuída uma vez cujo corpo é um só `return E`
vira `E` na chamada, com os argumentos no lugar dos parâmetros. `E` não pode ter
função, `this`, atribuição nem usar um parâmetro duas vezes. A ordem de
avaliação do JavaScript é mantida: se `E` avalia algo além de parâmetros antes
do último parâmetro, usa-os fora de ordem, sob condição ou deixa algum sem uso,
a troca só acontece onde os argumentos são simples (nomes, literais, `this`,
leituras de propriedade). Os nomes livres de `E` precisam ter o mesmo
significado na chamada: nome declarado como local em qualquer função é
recusado, e nome do topo de uma IIFE só vale dentro dela. A decisão segue uma
estimativa do tamanho minificado (corpo contra *callee*, mais a definição, que
sai quando ninguém mais a cita). Some quase todo o acessor do `dart:_rti`
(`Rti._getKind`, `_Utils.asRti`…). `DARTFORGE_JSPROD_INLINE=0` desliga.

Sem a estimativa o `limitless_ui` crescia 107 KB: corpos de 100 bytes
copiados em muitas chamadas. Com ela, o `01_print` perde ~5% e os projetos,
alguns KB.

### 10.4 Nomes privados do SDK sem uso

O prelúdio do SDK declara `var x = dart.privateName(lib, "_n")` para todo nome
privado da biblioteca, inclusive os dos estáticos, que são nomes JS comuns.
Sem uso, a linha ainda custava os bytes, e a *string* dela reservava o nome do
estático (toda *string* do runtime reserva), então `_getCanonicalRecipe` e
companhia ficavam por extenso. `own_sdk::drop_unused_private_names` tira as
linhas cuja variável ninguém mais cita. `DARTFORGE_JSPROD_PRIVATE_NAMES=0`
desliga. `01_print`: ~104.200 → 98.263.

### 10.5 Constantes que apelidam constantes

`static const String separatorString = _commaString;` agora dobra para `","`
como um inicializador literal, no emissor (`own_sdk::inline_constant`) e no
mundo (`is_inlinable_constant`), e os *getters* do `dart:_recipe_syntax` saem
do arquivo. −1 KB no `01_print`.

### 10.6 Classe que o runtime só compara não é instanciada

`JS_CLASS_REF(C)` entrega a classe a um *template* `JS()`, que pode estendê-la
ou construí-la, e o mundo instanciava `C`. Quando o *template* só compara o
buraco (logo depois de `instanceof`, ou junto de `===`/`!==`), `C` fica viva
só como tipo. O `# instanceof #` do `_isJsObject` com `RecordImpl` mantinha o
runtime dos *records* em todo programa, e com ele o `Object.hash` e a família
`SystemHash`. `01_print`: 98.263 → 91.772.

### 10.7 O enum de `JS_GET_NAME`/`JS_BUILTIN`

O emissor dobra `JS_GET_NAME(JsGetName.RTI_NAME)` num nome, mas o mundo
visitava o argumento e o tipo do *helper* (`String Function(JsGetName)`), e
enum referenciado é instanciado com todas as constantes: `JsGetName` e
`JsBuiltin`, com as listas `values`, iam em todo arquivo. Para os membros que o
emissor conhece, nenhum dos dois é visitado. `01_print`: → 88.445.

### 10.8 Resultado

Corpus (237 programas, soma):

| | bruto | gzip -9 |
| --- | ---: | ---: |
| antes | 33.147.610 | 9.301.768 |
| depois | **23.860.132** (−28%) | **7.577.459** (−18,5%) |
| `dart2js -O4` | 9.606.538 | 3.120.104 |

A mediana da razão nosso/`dart2js` caiu para 2,44× (bruto) e 2,37× (gzip). A
compilação mediana de um programa do corpus ficou em ~0,35 s.

| programa | antes | depois | `dart2js -O4` |
| --- | ---: | ---: | ---: |
| `01_print` | 128.592 | **88.445** | 34.926 |
| `120_convert_json` | 207.761 | **173.620** | 58.550 |
| `135_convert_json_classes` | 274.708 | **240.905** | 101.193 |
| `limitless_ui/example` | 4.096.297 (gz 1.158.021) | **4.030.541** (gz 1.148.341) | 3.708.482 (gz 983.819) |
| `new_sali/frontend` | 8.415.525 (gz 2.276.131) | **8.342.998** (gz 2.265.897) | 7.616.042 (gz 2.018.951) |

Os projetos foram compilados no macOS com o `package_config.json` reescrito
para a cache do pub local e os `*.template.dart` de `.dart_tool/build/generated`
copiados para uma árvore à parte; o `dart2js -O4` foi rodado sobre as mesmas
fontes (por isso o `limitless_ui` dá 3,71 MB, e não os 4,51 MB do
`build_web_compilers --release` da §1.1). Tempo: `new_sali` 27 → 30 s (o
*inline* faz algumas leituras a mais do arquivo), `dart2js` 68 s; `limitless_ui`
9 → 12 s, `dart2js` 21 s.

Validação: corpus `--production` 238/238 (desenvolvimento 238/238), testes de
`world`, `emit_js` e `emit_js_production`, e o `limitless_ui` percorrido no
Chrome *headless* (47 rotas) com o mesmo comportamento do binário anterior.
Essa varredura mostrou dois erros que **já existiam** antes desta rodada (o
mesmo resultado com o binário do começo da sessão): `NoSuchMethodError` em
`SafeElement|get#innerHtml` (`sanitize_dom`) em 45 das 47 rotas, e
`UnimplementedError` de um `isA` não rebaixado na rota `typeahead`. O e2e
26/26 da §6 precisa ser refeito.

### 10.9 O que falta, por ganho esperado

1. Os erros da §10.8 no `limitless_ui` (correção antes de tamanho).
2. Nos projetos, a diferença é código do programa no contrato do DDC
   (`new_sali` 1,10× bruto, 1,12× gzip): assinaturas (`set*Signature`), o
   `dart.fn` das *closures* sem necessidade de assinatura, checagens
   implícitas e *inlining* de *getters* triviais de instância.
3. No corpus, o piso: `_Universe`/`_Parser` do `dart:_rti` inteiros, o
   `JSNoSuchMethodError.toString` (sempre vivo, pela marca `TypeError`) que
   puxa `RegExp` e `FormatException`, o `Object.toString` universal que puxa o
   `_rtiToString` com `split`/`allMatches`. Cada corte é uma regra de precisão
   no mundo, como §10.6 e §10.7.
4. Enum com constante por constante viva (o mundo hoje vivifica todas junto
   com a classe).
5. Nomes de variável de tipo curtos nas receitas e regras (`|j.E` em vez de
   `Iterable.E`, ~1,3 KB no `01_print`) e as regras compactadas da sessão
   anterior (`"#": "|T|U"` para supertipos sem argumento, pequeno).

## 11. Rodada de 2026-10-13, tarde (macOS): os erros do `limitless_ui` e as regras repetidas

### 11.1 Os dois erros da §10.8 (desenvolvimento e produção)

Os dois aconteciam nos **dois** perfis (o desenvolvimento usa o mesmo
emissor), e o `dart2js -O4` das mesmas fontes passa limpo (47 rotas sem erro,
e2e 15/15). Havia um terceiro, só no desenvolvimento, escondido atrás do
primeiro.

1. **`SafeElement|get#innerHtml`** (45 de 47 rotas). O `sanitize_dom` declara
   `extension SafeElement on Element { @JS('innerHTML') external String get
   innerHtml; … }` numa biblioteca **sem** `@JS()`. O CFE só aceita membro
   `external` de `extension` sobre tipo de interop e o `JsUtilOptimizer` o
   rebaixa para leitura, chamada ou escrita direta no receptor
   (`js_util.getProperty`/`callMethod`/`setProperty`); a extensão não tem a
   função `Ext|get#m`. O emissor chamava a função que não existe.
   `Ctx::interop_extension_member` reconhece esses membros (nome do `@JS('x')`
   ou o nome Dart) e as quatro rotas (leitura, chamada, escrita simples e
   composta, `FnEmitter::extension_set`) escrevem `recv.innerHTML`,
   `recv.indexOf(…)` e `recv.x = v`, com o `jsInteropNullCheck` dos membros
   de interop. Na produção, o nome Dart de um `external` sem `@JS` fica
   reservado (`properties.rs`).
2. **`isA` não rebaixado** (rota `typeahead`, pelo `popper`):
   `testViewport.isA<JSObject>()`. `lower_is_a` só conhecia os primitivos e
   os tipos de interop do usuário; os de `dart:js_interop` caíam na função
   `isA`, que lança `UnimplementedError`. Agora sai como o DDC 3.6.2:
   `JSAny` é `v != null` (`true` com `JSAny?`), `JSObject`/`JSBoxedDartObject`
   são `instanceOfString(v, "Object")`, `JSFunction` é `"Function"`, `JSArray`
   `"Array"`, `JSPromise`, `JSArrayBuffer`, `JSDataView` e os *typed arrays*
   pelo nome do `@JS`, e `JSTypedArray` é `instanceof
   Object.getPrototypeOf(Int8Array)`.
3. **Estáticos de tipo de extensão de interop** (só desenvolvimento, todas as
   rotas com HTML sanitizado travavam): `switch (node.nodeType) { case
   Node.ELEMENT_NODE: … }` do `sanitize_dom` lia `L.Node.ELEMENT_NODE`, mas o
   objeto de suporte de um tipo de extensão de interop não tinha os campos
   estáticos (só os métodos). Todo nó caía no `default`, que o remove, e o
   `do { walk } while (houve modificação)` nunca terminava. A produção dobrava
   a constante e escapava. `emit_js_interop_class` agora emite os campos
   estáticos não `external` como os de um tipo de extensão apagado
   (`extension_type::emit_static_fields`).

Reprodução mínima: `corpus/js/239_interop_extension_members.dart` e
`240_interop_extension_type_statics.dart` (DDC como oráculo, a VM não compila
`dart:js_interop`). Corpus 240/240 em desenvolvimento e produção.

`limitless_ui` (varredura das 47 rotas no Chrome *headless*): produção 47/47
sem erro (antes 45 com erro), desenvolvimento 47/47. E2e (`ui_test/e2e` do
projeto, hoje 15 testes; o "26/26" da §6 era a suíte antiga): produção
**15/15** (o binário anterior dava 1/15), desenvolvimento **15/15**,
`dart2js -O4` 15/15.

### 11.2 Regras rti repetidas viram encaminhamento

`_Universe.findRule` e `_isSubtype` seguem uma regra que é *string* até a
regra da classe nomeada (`rti.dart`). Nos projetos ngdart quase toda classe de
*view* tem o mesmo corpo de regra (`{"EmbeddedView.T":"|vJ","|AG":[],…}`):
no `limitless_ui` eram 95 KB de corpos repetidos em 139 KB de regras.
`rti_rules::forward_identical` escreve `"|B":"|A"` quando o corpo de `B` repete
o de uma classe anterior no texto (cujo `addRules` já rodou), registrada uma
vez só. As chaves tocadas por `addOrUpdateRules` ficam como estão (ele não
aceita a forma de encaminhamento), e corpos curtos também: abaixo de 12 bytes
de ganho o gzip piorava mais do que o bruto melhorava (medido com 2, 12, 24 e
40). `DARTFORGE_JSPROD_RULE_FORWARDING=0` desliga.

### 11.3 Assinaturas sem o `_ti =>` e estáticos literais sem *getter* preguiçoso

* `rtiFromSignature`, `getSetterType` e `getMethodDefaultTypeArgs` só chamam a
  entrada da assinatura quando ela é função; a que não depende dos argumentos
  de tipo da instância (o texto não cita `_ti`) é o próprio rti. As de campo
  perdem `isConst` (só o depurador lê) e o `isFinal: false`.
* Campo estático, estático de tipo de extensão e variável de topo (não
  `late`) com inicializador literal (`null`, número, booleano, *string*
  simples) é atribuído na carga (`C.x = null`) em vez de passar pelo
  `dart.defineLazy`. Os nomes que uma função JS tem ou guarda (`length`,
  `name`, `caller`…) mantêm o *getter*. Num objeto de biblioteca, a escrita
  também deixa o achatamento (§9.2) trocar `N.x` por variável.

### 11.4 Tabela de nomes com codificação de prefixo

A tabela `t$N` (§9.5) escreve cada nome como um dígito (quantos caracteres
iniciais ele divide com o nome anterior) e o resto, decodificada uma vez na
carga. Os ids seguem a ordem do programa, e as *views* de um componente ficam
lado a lado: no `limitless_ui` a tabela vai de 50 para 28 KB. Só quando ganha
1 KB além do decodificador e os nomes são ASCII; nos programas do corpus fica
a lista simples (o gzip perdia um pouco). `corpus/js/241` confere
`runtimeType`, `toString` e erro de tipo pela tabela codificada.

### 11.5 Formas compactas e o *bootstrap*

* `S(C, () => ({__proto__: G(C.__proto__), m: …}))`, uma por tipo de
  assinatura de cada classe, vira `t$G(S, G, C, () => ({m: …}))`; o `t$G`
  põe o mesmo protótipo no objeto que o *thunk* monta (`shapes.rs`). 750 no
  `limitless_ui`, 17 no `01_print`.
* O prólogo de todo construtor genérico, `X.$ti = X.$ti || _ti ||
  dart.getReifiedType(X);`, vira `t$Q(X, _ti);` (1.152 no `limitless_ui`, 29
  no `01_print`). O `t$Q` chama a mesma função que as instruções chamavam (uma
  variável depois do achatamento): a mais comum, quando aparece 4 vezes ou
  mais.
* Inicializações seguidas de campo com `null` no construtor viram uma só,
  encadeada de trás para a frente (`this.c = this.b = this.a = null`), para
  as propriedades nascerem na mesma ordem.
* O *bootstrap* do SDK próprio criava cada símbolo `dartx` numa linha
  (`dartx.abs = Symbol("dartx.abs");`, 65 no `01_print`); agora é um laço
  sobre a lista de nomes, que continuam *strings* do runtime (o renomeio de
  propriedades as preserva). É o maior ganho do corpus nesta rodada (−432 KB).
* `Object.create` entrou na lista de funções puras do compressor: o objeto de
  biblioteca que o achatamento esvaziou perdia a declaração mas mantinha a
  chamada.

Medidos e descartados:

* juntar os `let` de um bloco no primeiro (`let e = …, n, r; …; n = …`): o
  bruto cai (−16 KB no `limitless_ui`, −13 KB no corpus), mas o gzip piora bem
  mais (+13 KB e +29 KB): a lista de declaradores é única em cada função, e os
  `let x = f(…)` repetidos comprimiam melhor;
* as sequências de ids sem nome na tabela `t$N` escritas como o comprimento
  (`"3 int  String"`): −61 KB brutos no corpus, mas +15 KB no gzip (os espaços
  seguidos já comprimiam quase de graça, e o decodificador entra em todo
  programa).

### 11.6 Resultado

Corpus, os mesmos 237 programas da §10.8 (macOS, `dart compile js -O4` do SDK
3.6.2 de macOS):

| | bruto | gzip -9 |
| --- | ---: | ---: |
| §10.8 | 23.860.132 | 7.577.459 |
| agora | **23.052.209** (−3,4%) | **7.488.242** (−1,2%) |
| `dart2js -O4` | 9.606.538 | 3.120.104 |

| alvo | §10.8 | agora | `dart2js -O4` |
| --- | ---: | ---: | ---: |
| `01_print` | 88.445 (gz 28.373) | **85.454** (gz 28.047) | 34.926 (gz 11.543) |
| `120_convert_json` | 173.620 | **167.191** | 58.550 |
| `135_convert_json_classes` | 240.905 | **232.822** | 101.193 |
| `limitless_ui/example` | 4.030.541 (gz 1.148.341) | **3.857.125** (gz 1.136.359) | 3.708.482 (gz 983.819) |
| `new_sali/frontend` | 8.342.998 (gz 2.265.897) | **8.024.157** (gz 2.242.510) | 7.616.042 (gz 2.018.951) |

Os projetos ficaram a 1,04× (`limitless_ui`) e 1,05× (`new_sali`) do
`dart2js` no bruto; no gzip, 1,16× e 1,11×. No corpus a mediana da razão
caiu para 2,35× (bruto) e 2,34× (gzip): o piso do runtime domina.

Tempo de compilação (macOS): `limitless_ui` 10–12 s (`dart2js` 21 s),
`new_sali` 24–28 s (`dart2js` 68 s); mediana do corpus ~0,35–0,5 s por
programa.

Validação depois de cada passo: corpus diferencial 241/241 em
desenvolvimento e produção; `limitless_ui` com as 47 rotas sem erro no Chrome
e o e2e 15/15; `new_sali` carrega no Chrome sem erro, com a mesma página do
`dart2js`.

### 11.7 O que falta, por ganho esperado

1. Assinaturas de método das *views* (`_handle_*` presos por `dart.bind`) e o
   `dart.fn` das *closures*: ~60 KB e ~15 KB no `limitless_ui`. Só saem com
   uma regra no mundo que prove que o tipo da função nunca é observado (`is`/
   `as` de tipo função, covariância de genérico, `runtimeType`, `toString`,
   `dcall`); o DDC calcula esse tipo pela assinatura quando alguém pergunta.
2. O piso do runtime (§10.9 item 3): `dart:_rti` é 30% do `01_print` não
   minificado (`_Universe` 21 KB, `_Parser` 9,5 KB, `Recipe` 5 KB). O
   `JSNoSuchMethodError` (extensão registrada para `TypeError`) mantém
   `RegExp`/`FormatException` vivos de verdade: só sai com outra
   implementação da mensagem. Formas compactas do *bootstrap* como a do
   `dartx` rendem mais no corpus que nos projetos (cada programa repete o piso).
3. Enum com constante por constante viva (~10 KB no `limitless_ui`).
4. Os itens pequenos da §10.9 (nomes de variável de tipo curtos, regras
   compactadas) e o `addRtiResources(C, ["|x"])` de um item só (~2 KB por
   programa pequeno, pede mudar a função do runtime).

## 12. Rodada de 2026-10-13, fim da manhã (macOS): o piso do runtime de verdade

Medido antes de mexer (o binário do fim da §11): `void main() { print('hi'); }`
saía com **84.695 bytes**, contra **5.076** do `dart2js -O4`. O piso não era o
`dart:_rti` sozinho: o mundo mantinha o caminho inteiro do `noSuchMethod`
(`InvocationImpl`, `Map.fromIterable`, as classes de mapa), as expressões
regulares e seus iteradores, por três ou quatro chamadas dinâmicas do
runtime. Num corpus de 241 programas isso é 80 KB × 241. Cada passo foi
medido sozinho (bruto e gzip, corpus, `01_print`, `limitless_ui`, `new_sali`)
e validado com o corpus diferencial 241/241 nos dois perfis.

### 12.1 Chamada com argumento de parâmetro sem uso (`inline.rs`)

O `checkBinding(old, new, expression, location)` do ngdart só compara os dois
primeiros em produção, mas recebia a *string* da expressão e a localização em
3.370 chamadas do `limitless_ui`. O *inline* recusava a função porque os
parâmetros opcionais têm valor padrão (`n = null`). Agora um parâmetro que o
corpo não lê pode ter valor padrão e pode faltar na chamada; o argumento dele
precisa ser nome, literal ou `this` (some junto). Argumento que é leitura de
propriedade deixou de ser descartado (um *getter* pode rodar ou lançar), e o
nome livre `arguments` é recusado. `limitless_ui` −78 KB brutos, −22 KB gzip;
`new_sali` −79 KB e −22 KB.

### 12.2 `x++` com valor descartado vira `++x` (`emit_js`)

Instrução de expressão e atualização de `for` não precisam do valor antigo: o
temporário (`t$0 = i, i = t$0 + 1, t$0`) some, e com ele o `let t$0`. Corpus
−131 KB brutos e −105 KB gzip (afeta os dois perfis).

### 12.3 Constantes de enum uma a uma (`world`, `module.rs`)

O enum referenciado criava todas as constantes. Agora cada constante nasce
quando é lida, e todas quando o `values` sintético está vivo; o emissor deixa
de fora o *getter* preguiçoso das mortas e do `values`. `new_sali` −34 KB
brutos, −12 KB gzip; `limitless_ui` −2,4 KB.

### 12.4 Promoção dos dois lados de `a && b` (`expr.rs`)

As promoções do operando direito eram descartadas: em `if (t != '' && m !=
null) m(t)`, `m` continuava anulável e uma chamada por *typedef* antigo
(`typedef String M(String s)`) virava `dart.dcall`. As promoções negadas de
`a && b` agora são apagadas (um `&&` falso não diz nada de cada lado). Era o
`dcall` do `_StackTrace.toString` do runtime.

### 12.5 Remendos do SDK de produção (`emit_js/src/sdk_patches.rs`)

O perfil de produção compila o SDK; três funções do runtime do DDC usavam
receptor `dynamic` e mantinham vivos `dart.dload`/`dsend`, e com eles o
`noSuchMethod` inteiro:

* `diagnoseIndexError(indexable, index)` lia `indexable.length`: todo chamador
  passa um indexável do JavaScript, cujo `length` é o do Dart;
* `stringContainsUnchecked` chamava `other.allMatches(...)`: `other` é o
  `Pattern` de `String.contains`;
* `JSNoSuchMethodError` (a extensão do `TypeError`, viva em todo programa)
  montava a mensagem com quatro `RegExp` estáticas e cortava a pilha com
  `String.split`: agora roda as mesmas expressões do JavaScript
  (`new RegExp(fonte).exec(mensagem)`, que é o que o `firstMatch` faz) e o
  `split` do JavaScript. As mensagens são as mesmas (conferido com
  `js_util.callMethod` sobre valor que não é função e sobre `undefined`).

Os arquivos remendados entram pela sobreposição do `SdkLayout` (o conteúdo
troca, o caminho fica), escritos uma vez por conteúdo em
`target/dartforge/js-sdk-patches/`; a chave do cache do SDK cobre o
conteúdo. Remendo cujo texto não aparece (outro SDK) é pulado.
`DARTFORGE_JSPROD_SDK_PATCHES=0` desliga. `print('hi')`: 84,7 → 52,0 KB;
corpus 23,24 → 17,06 MB.

### 12.6 O verificador e os nomes de *template*

* O leitor de seletores do verificador (`text_selectors`) não pulava
  comentários: o apóstrofo de `// there's no guarantee…` num *template*
  `JS()` abria uma *string* até a próxima aspa, e os usos no meio sumiam.
  Apareceu quando o `_bind` do rti deixou de estar vivo por outro caminho
  (o `Future.then` quebrava com `[_bind] is not a function`).
* Nome privado usado pelo símbolo do prelúdio (`x[_bind]` com
  `privateName(dart_rti, "_bind")`) revive o membro daquela biblioteca mesmo
  que o mesmo nome esteja restrito a outro cone.
* Um nome escrito num *template* `JS()` (`#.split(#)`) é propriedade do
  JavaScript com chave *string*: não alcança os membros das classes nativas
  que o emissor chaveia por símbolo `dartx` ou privado (`JSString`, `JSArray`,
  *typed data*, DOM). O mundo registra esses nomes à parte
  (`Engine::new_template_selector`), e o verificador só revive membro dessas
  classes por uso simbólico (`[$nome]`, símbolo privado, nome de
  `dsend`/`dload`).

### 12.7 Regras rti e recursos do programa em uma chamada (`rti_rules.rs`)

* Cada biblioteca registrava as regras das suas classes no seu `JSON.parse`,
  repetindo as das classes que compartilham (as *views* e diretivas do
  ngdart) e comprimindo em separado: 179 chamadas no `limitless_ui`. O
  `addRules` é um `Object.assign` nas regras do universo, e carregar uma
  biblioteca não testa tipo; uma chamada só, no lugar da última, com o valor
  mais tardio de cada chave, deixa as mesmas regras antes do `main`. Chave
  tocada por `addOrUpdateRules` do programa mantém tudo como está.
  `limitless_ui` −36 KB brutos, −8,7 KB gzip. `DARTFORGE_JSPROD_MERGE_RULES=0`
  desliga.
* A partir de 32 instruções, os `dart.addRtiResources(L$lib.C, [...])` do
  programa viram uma chamada sobre uma lista plana (`C, "|a", D, ["|b",
  "|c"]`), no lugar da última. Classe que o `applyMixin` ou o
  `registerExtension` recebe fica onde está: eles copiam as marcas na carga
  (com o limiar em 2 o corpus mostrou três programas de *mixin* quebrados
  antes dessa exceção, e 241/241 depois). Abaixo de 32 o laço custa mais do
  que economiza. `limitless_ui` −8,6 KB e −3,7 KB gzip; `new_sali` −18,7 KB e
  −7,5 KB. `DARTFORGE_JSPROD_BATCH_RESOURCES=0` desliga.

### 12.8 Resultado

Corpus (240 programas com saída do `dart2js -O4`; o 229 é negativo):

| | bruto | gzip -9 | mediana nosso/`dart2js` (bruto, gzip) |
| --- | ---: | ---: | ---: |
| §11.6 | 23.372.702 | 7.586.457 | 2,35×, 2,34× |
| agora | **17.061.534** (−27%) | **5.664.590** (−25%) | **1,64×, 1,72×** |
| `dart2js -O4` | 9.760.610 | 3.167.516 | |

| alvo | §11.6 | agora | `dart2js -O4` |
| --- | ---: | ---: | ---: |
| `print('hi')` | 84.695 | **52.007** | 5.076 |
| `01_print` | 85.454 (gz 28.047) | **60.113** (gz 20.207) | 34.926 (gz 11.543) |
| `120_convert_json` | 167.191 | **159.246** | 58.550 |
| `135_convert_json_classes` | 232.822 | **227.638** | 101.193 |
| `limitless_ui/example` | 3.857.125 (gz 1.136.355) | **3.726.961** (gz 1.099.111) | 3.708.482 (gz 983.819) |
| `new_sali/frontend` | 8.024.157 (gz 2.242.506) | **7.854.243** (gz 2.188.550) | 7.616.042 (gz 2.018.951) |

Os projetos ficaram a 1,005× (`limitless_ui`) e 1,03× (`new_sali`) do
`dart2js` no bruto; no gzip, 1,12× e 1,08×.

Validação: corpus diferencial 241/241 em desenvolvimento e produção depois de
cada passo; `limitless_ui` com o e2e 15/15 e as 47 rotas sem erro nos dois
perfis; `new_sali` carrega no Chrome sem erro.

Medidos e descartados nesta rodada:

* o lote dos `addRtiResources` sem limiar: o corpus crescia 5 KB brutos e
  3 KB gzip (o laço em todo programa pequeno);
* tirar o `dart.fn` das *closures* e as assinaturas de método só com uma
  regra de mundo grossa: o `allowInterop` do DDC exige o rti anexado
  (`isDartFunction`), e o `toString` de uma *closure* mostra o tipo, então
  qualquer função que chegue a `Object` precisa dele; a regra pede análise de
  fluxo de valores de função, que não coube.

### 12.9 O que falta, por ganho esperado

1. **Nomes dos campos `late`** (contrato): o `dart.lateFields(proto, {chave:
   "nome"…})` das *views* custa 95 KB brutos no `limitless_ui` (645 chamadas);
   tirar os nomes economiza ~64 KB brutos e ~19 KB gzip. O `dart2js -O2` já
   faz isso (`omitLateNames`, `options.dart:1097-1100`), mas a mensagem do
   `LateInitializationError` muda: decisão do dono.
2. **O piso que sobra** (`print('hi')` 52 KB): é quase todo `dart:_rti`
   (`_Universe` 21 KB, `_Parser` 15 KB não minificados), vivo porque o
   *bootstrap* avalia receitas e regras na carga e o `Object.toString` chega
   ao `_rtiToString`. Os operadores das classes nativas (`JSNumber` inteiro)
   ficam vivos pela regra "operador é sempre vivo" do mundo; cortá-los pede
   registrar os seletores de operador das chamadas dinâmicas (`dsend`,
   `dindex`).
3. Assinaturas de método e o `dart.fn` das *closures* (§11.7, item 1), com a
   análise de fluxo de função descrita acima (~14 KB gzip de assinaturas no
   `limitless_ui`).
4. O `polyfill` do DOM (2,7 KB não minificados) em todo programa, mesmo sem
   `dart:html`.

## 13. Rodada de 2026-10-13, tarde (macOS): nomes de propriedade compartilhados

Medido antes de mexer (o binário do fim da §12): `print('hi')` 52.007 bytes,
`limitless_ui` 3.726.961 (gz 1.099.111), `new_sali` 7.854.243 (gz 2.188.550).
Cada passo foi validado sozinho com o corpus diferencial 241/241 nos dois
perfis (e o `corpus/modern` 26/26 a partir da §13.6), o `limitless_ui` com o
e2e 15/15 e as 47 rotas sem erro no Chrome, e o `new_sali` carregando sem
erro.

### 13.1 Nomes de propriedade compartilhados (`properties.rs`, `minify.rs`)

A saída do `dart2js` tem 8.331 nomes de propriedade distintos no
`limitless_ui`; a nossa tinha 20.722, e 14.773 deles com três caracteres
(`e.wje`, `e.KZt`): cada campo de *view* (`_el_0`, `_expr_3`) é um nome
privado por biblioteca, e o renomeio dava um nome curto distinto a cada um.
O `dart2js` dá aos campos nomes por posição na classe (`a`, `b`, `c`), e
classes sem relação repetem os mesmos.

Agora dois nomes renomeados podem receber o mesmo nome curto quando nenhum
objeto pode carregar os dois. Do modelo de elementos sai, para cada classe
sem subtipos, o **grupo de coabitação**: os nomes JS dos membros de
instância de toda a cadeia de supertipos (superclasses, *mixins*,
interfaces — um encaminhador de `noSuchMethod` põe os membros da interface
na classe — e `on`), na grafia pública e na privada/armazenamento do perfil
de produção. Nomes de estáticos, construtores, membros de topo, classes e
os *helpers* do runtime são **solitários** (vivem em funções construtoras e
objetos de biblioteca), e um nome renomeado que nenhum grupo conhece também
é tratado como solitário. A atribuição continua por frequência; um nome
compartilhável recebe o menor alvo que nenhum nome dos seus grupos e nenhum
solitário usa. Os nomes que chegam ao runtime como *string* (`dsend`,
`dload`, `in`) continuam reservados, como antes.

`limitless_ui` −88 KB brutos e **−44 KB gzip**; `new_sali` −118 KB e
**−70 KB gzip**: as *views* de bibliotecas diferentes passam a se escrever
com as mesmas letras, e o gzip as encontra. Quase todo nome de propriedade
fica com um ou dois caracteres. `DARTFORGE_JSPROD_SHARED_NAMES=0` desliga.

### 13.2 Remendos do piso (`sdk_patches.rs`)

* `getThrown` (vivo em todo programa com `catch`) reconhecia o estouro de
  pilha com `message.contains('call stack')`; o caminho genérico de
  `String.contains` (o argumento é um `Pattern`) mantinha `allMatches`, os
  iteráveis de casamento, `Iterable` e `StringBuffer`. O teste com a
  *string* literal é o `includes` do JavaScript.
* `getReifiedType` chamava `getRtiForRecord` para um `RecordImpl`, e com ele
  `_Universe.bind` e os rtis de ligação. A conta virou um *getter* de
  `RecordImpl` (`_reifiedRti`), vivo só onde um *record* é criado.

`print('hi')` 52,0 → 49,0 KB. `DARTFORGE_JSPROD_FLOOR_PATCHES=0` desliga.

### 13.3 Operadores vivos por seletor (`world`, `verify.rs`)

O mundo mantinha todo operador de toda classe instanciada (o `JSNumber` e o
`JSString` inteiros em todo programa). Com o SDK no mundo, os usos binários,
unários, de índice, compostos e de padrão (`< 5`, listas, mapas) registram o
seletor do operador restrito ao receptor; os operadores de número que o
emissor escreve como operador do JavaScript (`+ - * / & | ^ < > <= >=` com
número à esquerda e número ou `dynamic` à direita, `-x`, `~x`) não registram
nada. O verificador passou a ler os usos de operador no texto
(`x[$modulo]`, pelo prelúdio `var $modulo = dartx["%"]`, e `a["+"](b)`) como
rede de segurança; no corpus ele não precisou corrigir nenhum.
`print('hi')` 49,0 → 47,8 KB. `DARTFORGE_JSPROD_OPERATOR_SELECTORS=0`
desliga.

### 13.4 O *polyfill* do DOM só com `dart:html`

O `dart.global` chamava o `polyfill` do DDC (`NodeList.prototype.get`, os
construtores `AudioContext`/`FontFaceSet`… que as classes nativas do
`dart:html` registram) em todo programa. O mundo segura o `polyfill` de
`dart:_runtime` até uma função do `dart:html` ou da família
(`dartforge_world::DOM_LIBRARIES`) ficar viva, e sem isso a chamada sai do
texto do `global`. `print('hi')` 47,8 → 46,3 KB; o `limitless_ui` (que usa
`package:web`) perde 1,5 KB; o `new_sali` (`dart:html`) mantém.
`DARTFORGE_JSPROD_DOM_POLYFILL=0` desliga.

### 13.5 Sem os nomes dos campos `late` (decisão do dono)

Como o `dart2js -O2` em diante (`omitLateNames`), o perfil de produção
escreve `LateInitializationError: Field '' has not been initialized.` (e
`Local ''`, `has already been initialized`), conferido contra a saída do
`dart2js -O2` para campos, variáveis de topo e locais. Os mapas do
`dart.lateFields` levam `0` no lugar do nome. O desenvolvimento continua
idêntico à VM. `DARTFORGE_JSPROD_LATE_NAMES=0` mantém os nomes.

O diferencial compara a produção com a referência com essa única troca
aplicada às linhas de `LateInitializationError` (`report::omit_late_names`),
desligada quando o compilador mantém os nomes, e o relatório nomeia cada
programa em que ela valeu (`ok 56_late (late names omitted in production,
as dart2js -O2)`). No corpus, só o `56_late` imprime a mensagem.
`limitless_ui` −66 KB brutos e −22 KB gzip; `new_sali` −68 KB e −20 KB.

### 13.6 Correções no caminho

* Constante de enum citada por atalho de ponto (`opcional(.vermelho)`) não
  nascia: desde a §12.3 as constantes vivem uma a uma, e o `static_member`
  do mundo só olhava as funções estáticas (`corpus/modern/330`).
* O teste ignorado `tests/identity.rs` do `emit_js_production` (sem filtro e
  com `AllLive` emitem o mesmo texto) quebrou com os literais estáticos
  ansiosos da §11.3: as escolhas de emissão do perfil de produção que não
  são poda perguntam `Liveness::production`, e o `AllLive` diz que não.

### 13.7 Sem `_ti` para classe sem parâmetros de tipo próprios (`ctx.rs`)

Uma classe sem parâmetros de tipo tem rti fechado mesmo sob `extends
S<int>` (as *views* do ngdart: `_ViewX1 extends EmbeddedView<X>`), e o
`instanceType` o acha pelo construtor. O construtor dela deixa de receber
`_ti`, e criá-la deixa de avaliar a receita (`new Vt.o(J("|xR"), e, t)`, 943
fábricas de *view* no `limitless_ui`); o construtor da superclasse genérica
recebe `null` e guarda o rti da classe em `$ti`, como antes. Só no perfil de
produção (`Ctx::production`). `limitless_ui` −26 KB brutos e −5,7 KB gzip;
`new_sali` −42 KB e −8,5 KB. `DARTFORGE_JSPROD_CLOSED_TI=0` desliga.

### 13.8 Resultado

| alvo | §12.8 | agora | `dart2js -O4` |
| --- | ---: | ---: | ---: |
| `print('hi')` | 52.007 (gz 17.996) | **46.336** (gz 16.169) | 5.076 |
| `01_print` | 60.113 (gz 20.207) | **55.346** (gz 18.771) | 34.926 (gz 11.543) |
| `120_convert_json` | 159.246 | **155.706** | 58.550 |
| `135_convert_json_classes` | 227.638 | **224.481** | 101.193 |
| `limitless_ui/example` | 3.726.961 (gz 1.099.111) | **3.546.977** (gz **1.026.260**) | 3.708.482 (gz 983.819) |
| `new_sali/frontend` | 7.854.243 (gz 2.188.550) | **7.626.767** (gz **2.082.024**) | 7.616.042 (gz 2.018.951) |

Corpus (240 programas com saída do `dart2js -O4`): 17.061.534 / 5.664.590 →
**15.987.460 / 5.329.837** (bruto / gzip), contra 9.760.610 / 3.167.516; a
mediana da razão nosso/`dart2js` caiu de 1,64×/1,72× para **1,51×/1,58×**.

O `limitless_ui` ficou **menor que o `dart2js -O4` no bruto** (0,956×) e a
1,043× no gzip (era 1,12×); o `new_sali` a 1,001× no bruto e 1,031× no gzip
(era 1,08×).

Medidos e descartados:

* o `dart.fn` das *closures* e as assinaturas de método com uma análise de
  fluxo de valores de função (§12.9, item 3): o limite superior, tirando
  **todo** `dart.fn` do código do programa sem olhar a correção, é −16 KB
  brutos e −5,2 KB gzip no `limitless_ui`, e as assinaturas que sobram já são
  só as dos nomes usados dinamicamente (`dart.bind`, `dsend`). Não paga o
  risco de uma análise que precisa provar que o rti nunca é observado.

### 13.9 O que falta, por ganho esperado

1. No gzip dos projetos, o que resta medido por ablação no `limitless_ui`:
   as receitas rti (4.992 *strings*, ~17 KB gzip), a tabela de nomes `t$N`
   (~8 KB gzip, necessária para `runtimeType`/`toString` com o nome da
   classe; o `dart2js -O4` mostra nomes minificados) e as regras rti (~9 KB
   gzip somando runtime e programa).
2. O piso (`print('hi')` 46 KB): `Object.toString`, `runtimeType` e
   `JSArray.toString` (sempre vivos pelo `str()` dinâmico) mantêm o
   `_Universe`/`_Parser` e o `StringBuffer`; cortar pede saber que nenhum
   objeto sem `toString` próprio chega ao `str()`, o que é inferência de
   tipos de argumento à moda do `dart2js`.
3. Os `let` por declaração (68 KB brutos no `limitless_ui` contra 22 KB de
   `var` do `dart2js`) — juntá-los piorou o gzip na §11.5.

## 14. Rodada de 2026-10-13, fim da tarde (macOS): onde está o gzip que falta

Medido antes de mexer (o binário do fim da §13, sem `-O4`, como nas rodadas
anteriores): `limitless_ui` 3.546.977 (gz 1.026.260), `new_sali` 7.626.767
(gz 2.082.025), contra 3.708.482 (gz 983.819) e 7.616.042 (gz 2.018.951) do
`dart2js -O4`.

### 14.1 Ablação por categoria no `limitless_ui`

Cada linha é o gzip que some quando a construção sai do arquivo minificado
(medido sozinho; as somas não são aditivas):

| construção | ocorrências | bruto | gzip |
| --- | ---: | ---: | ---: |
| assinaturas (`t$G(…)`; métodos 419, campos 155, *getters* 152, *setters* 22) | 748 | 43.170 | 13.952 |
| JSON das regras rti (programa 25 KB + SDK 16 KB; o do `dart2js` tem 46 KB e custa 5,3 KB gzip) | 2 | 41.141 | 8.549 |
| tabela de nomes `t$N` | 1 | 28.165 | 7.670 |
| lista dos `addRtiResources` em lote | 1.693 classes | 15.697 | 7.276 |
| nomes sem inicializador no `var` do topo (achatamento) | 3.502 | 11.635 | 6.487 |
| `t$P(C.x = function…, C)` (o embrulho) | 1.635 | 9.812 | 5.537 |
| `t$R` de receitas usadas uma vez | 691 | — | 4.849 |
| `dart.fn` das *closures* | 1.881 | 13.371 | 4.274 |
| bloco dos `t$T` (receitas fixas) | — | 7.505 | 2.642 |
| embrulhos IIFE por biblioteca | 492 | — | 529 |

Por categoria de *token* (acorn), a diferença para o `dart2js` está nos
identificadores (+37 KB gzip; os nomes do topo achatados são identificadores
aleatórios, e o `dart2js` escreve `A.x`, que conta como propriedade: somando
identificadores e propriedades, +17 KB), na pontuação (+27 KB) e nas
palavras-chave (+8 KB: `let` por declaração, `get`, `this`, `new`). As
*strings* custam o mesmo nos dois.

### 14.2 Classe e construtor padrão numa chamada (`shapes.rs`)

`C = class …{…};` seguido de `(C.new = function(…){…}).prototype =
C.prototype;` vira `C = t$C(class …{…}, function(…){…})`, com `t$C = function(c,
f) { c.new = f; f.prototype = c.prototype; return c; }`: o nome da classe sai
uma vez em vez de três. A função do construtor passa a ser criada antes da
atribuição de `C`, o que ninguém observa (só é chamada depois). Só quando as
duas instruções são vizinhas e o construtor é o `new`; os outros continuam no
`t$P`. `DARTFORGE_JSPROD_CLASS_CONSTRUCTOR=0` desliga.

| alvo | antes | agora | `dart2js -O4` |
| --- | ---: | ---: | ---: |
| `limitless_ui/example` | 3.546.977 (gz 1.026.260) | **3.537.433** (gz **1.020.554**) | 3.708.482 (gz 983.819) |
| `new_sali/frontend` | 7.626.767 (gz 2.082.025) | **7.602.291** (gz **2.067.576**) | 7.616.042 (gz 2.018.951) |

O `new_sali` ficou **menor que o `dart2js -O4` no bruto** (0,998×) e a 1,024×
no gzip; o `limitless_ui` a 0,954× e 1,037×. Validação: corpus diferencial
241/241 em desenvolvimento e produção, `corpus/modern` 26/26 nos dois,
`limitless_ui` com o e2e 15/15 e as 47 rotas sem erro, `new_sali` carrega sem
erro.

### 14.3 Medidos e descartados (ou adiados)

* Deduplicação de *strings* (§9.x) com limiares: só a partir de 3 usos, −2,2 KB
  gzip e +16 KB brutos; a partir de 5 usos ou com usos a mais de 100 KB de
  distância, −3,7 KB gzip e +26 KB brutos. Troca bruto por gzip; fica para
  decidir com o corpus. Ordem das declarações por primeira ocorrência: pior
  (+0,9 KB). Sem deduplicação: +8 KB gzip.
* Receitas fixas (`t$T`) por primeira ocorrência ou só com 3 usos: neutro ou
  pior.
* Regras rti na ordem do programa (as chaves hoje saem em ordem alfabética,
  pelo `serde_json::Map`): −1 KB gzip, estimado sobre o arquivo final.
* Lista dos `addRtiResources` com o id próprio implícito (o número do id é
  quase sempre o anterior mais 1; 1.322 de 1.693): −8,3 KB brutos e −2,3 KB
  gzip, estimado.

### 14.4 O que falta, por ganho esperado

1. Assinaturas (14 KB gzip): os `dart.bind` de *tearoff* com receptor de tipo
   estático conhecido (1.815 no `limitless_ui`, quase todos `_handleEvent_*`
   das *views*) poderiam levar o tipo na chamada (`bind(obj, nome, método)`
   já recebe o método); a entrada da assinatura ficaria só para os nomes de
   `dsend`/`dload`. Pede provar no mundo que nenhuma subclasse viva
   sobrescreve o método com outro tipo.
2. Os nomes do topo sem inicializador (6,5 KB): declarar no lugar da
   atribuição pede tirar as IIFEs de biblioteca (renomeando os locais delas).
3. Estender o `t$C` com os recursos rti da classe (o nome sai da lista em
   lote) e com as assinaturas.
4. Os itens estimados da §14.3 (ordem das regras, ids implícitos, limiar da
   deduplicação).

## 15. Rodada de 2026-10-27 (macOS): os dois projetos abaixo do `dart2js -O4`

Medido antes de mexer (o `main` em `45327774`, macOS arm64, Node 24.21,
SDK 3.6.2; `gzip -9`). O `dart2js -O4` dos projetos foi refeito hoje com o
`build_runner build --release` (`dart2js_args: ["-O4"]`), sobre os mesmos
*commits* dos projetos de antes:

| alvo | nosso | `dart2js -O4` |
| --- | ---: | ---: |
| `limitless_ui/example` | 6.579.927 (gz 1.595.453) ¹ | 3.704.125 (gz 982.814) |
| `new_sali/frontend` | 7.634.602 (gz 2.071.615) | 7.614.136 (gz 2.018.523) |
| corpus (273 programas com saída do `dart2js`) | 18.852.978 (gz 6.198.985) | 11.024.980 (gz 3.582.721) |
| mediana nosso/`dart2js` no corpus | 1,574× (gz 1,621×) | |
| `print('hi')` | 47.555 (gz 16.431) | 5.074 (gz 1.801) |

¹ O ponto fixo do mundo não convergia em 40 rodadas e o arquivo saía sem
poda (§15.1). Corrigido isso, o ponto de partida real era 3.610.318 (gz
1.036.655).

Cada passo abaixo foi validado sozinho: corpus diferencial `corpus/js`
274/274 em desenvolvimento e produção e `corpus/modern` 26/26 em produção
(24/26 em desenvolvimento, as mesmas duas falhas de antes, §15.12), testes
e `clippy` dos *crates* tocados. Os passos que mudam o que vive ou como as
*tear-offs* e os metadados rodam passaram também pelo e2e do `limitless_ui`
(15/15), pelas 48 rotas sem erro no Chrome e pelo `new_sali` carregando sem
erro (o mesmo HTML montado, 6.087 caracteres, que o do `dart2js`). Cada
otimização tem o seu `DARTFORGE_JSPROD_*=0`.

### 15.1 `callAsFunction` do `dart:js_interop` (`emit_js`, `ctx.rs`)

O `@JS('call') external callAsFunction` do `JSFunctionUtilExtension` não tem
*patch*: o CFE troca cada chamada por `f.call(…)`. O emissor deixava os
membros `external` do SDK fora dos membros de extensão de interop, e a chamada
ia para um `js_interop["JSFunctionUtilExtension|callAsFunction"]` que nunca é
definido; em produção o verificador o reenraizava a cada rodada, o ponto fixo
do `limitless_ui` não fechava (o `sanitize_dom` o usa) e o arquivo saía sem
poda (6,6 MB). Os nomes `@JS` dos membros `external` de extensão do
`dart:js_interop` agora são lidos como os do programa, nos dois perfis
(suíte `tests/language`: 2756/2863, sem regressão).

### 15.2 Membros de `Object` e acessos estáticos como seletores (`world`)

* Os membros de `Object` (`toString`, `==`, `hashCode`, `noSuchMethod`,
  `runtimeType`) deixaram de ser vivos em toda classe instanciada. O runtime
  os chama por `extensionSymbol('toString')` (`dart.str`, `dart.equals`…),
  que agora registra o seletor quando a função do runtime vive; quando uma
  biblioteca de interop vive (o JavaScript pode segurar qualquer objeto Dart)
  todos voltam a valer e o `core.Object` é instanciado. A porta é a de
  interop, não a dos nativos: `dart:typed_data` só cria nativos.
  `DARTFORGE_JSPROD_OBJECT_SELECTORS=0`.
* `C.nome` e `prefixo.nome` não registram seletor de instância (o acesso é
  estático). `DARTFORGE_JSPROD_STATIC_SELECTORS=0`.
* `bind` nos *templates* `JS()` e no texto do runtime (`#.bind(#)` do
  `dart.bind` e do *patch* de `async`) é o `Function.prototype.bind`, não o
  `Converter.bind`: ele mantinha vivo todo `bind` de `Converter`/
  `StreamTransformer`, e com eles a maquinaria de *streams*, em todo programa
  com `dart:convert`. `jsonEncode([1, 2])`: 134 → 84 KB.
  `DARTFORGE_JSPROD_FUNCTION_METHODS=0`.
* Um identificador resolvido para local ou parâmetro não mantém vivos os
  estáticos de mesmo nome (`int hash = …` em `Object.hashAll` mantinha
  `Object.hash` e o `SystemHash` inteiro). `DARTFORGE_JSPROD_LOCAL_NAMES=0`.
* O inicializador de um campo `late` de instância é percorrido com o
  *getter* implícito do campo (o emissor o calcula ali, na primeira leitura),
  não na instanciação da classe: os `late` não lidos do `_Uri`
  (`pathSegments`, `queryParametersAll`…) mantinham UTF-8, Base64 e
  `Map.unmodifiable` em todo programa que faz `Uri.parse`.
  `DARTFORGE_JSPROD_LATE_INITIALIZERS=0`.

### 15.3 Remendos do piso (`sdk_patches.rs`, `DARTFORGE_JSPROD_FLOOR_PATCHES`)

* `trackCall` (atrás de todo `dload`/`dput`/`dsend`) amostra chamadas para o
  depurador do DDC só com `dart.__trackProfile`, que só o carregador de
  desenvolvimento liga: em produção o corpo fica vazio.
* `InvocationImpl` montava os argumentos nomeados com
  `Map.unmodifiable(Map.fromIterable(…))`, cuja fábrica de `LinkedHashMap`
  mantinha `CustomHashMap`, `IdentityMap` e o preenchimento por iterável; um
  mapa literal no mesmo `UnmodifiableMapView` tem as mesmas chaves, ordem e
  tipo (conferido contra o perfil sem o remendo). −5 a −6 KB nos programas
  com chamadas dinâmicas.
* `_rtiToString` cortava o prefixo da biblioteca com `String.indexOf`, cujo
  caminho de `Pattern` mantinha `JSSyntaxRegExp`, `matchAsPrefix` e
  `StringMatch` em quase todo programa; o separador é uma *string* e
  `_Utils.stringIndexOf` é o mesmo `indexOf` do JavaScript. Corpus −325 KB.

### 15.4 Prelúdios içados para fora das IIFEs (`own_sdk.rs`)

Cada IIFE de biblioteca declarava os seus apelidos de `dartx` e os símbolos
privados do SDK (`var $add = dartx.add`, `var _is = dart.privateName(…)`):
474 cópias das mesmas sete linhas no `limitless_ui`, e o mesmo símbolo com um
nome minificado diferente em cada biblioteca. Os apelidos que valem o mesmo
em todo módulo viram uma declaração depois do *bootstrap*; um nome com dois
valores fica local. `limitless_ui` −16 KB brutos e **−13 KB gzip**: o gzip
passa a ver o mesmo nome em toda biblioteca. `DARTFORGE_JSPROD_HOIST_PRELUDES=0`.

### 15.5 IIFEs desembrulhadas e nomes declarados na primeira atribuição (`unwrap.rs`)

Com os prelúdios içados e os membros achatados, quase toda IIFE de biblioteca
só atribui nomes do arquivo. Ela dá lugar ao seu corpo — também quando
declara nomes que não aparecem em nenhum outro lugar — e os nomes que o
achatamento declarava sem valor no topo (`var a, b, c, …`: 3.500 nomes
aleatórios no `limitless_ui`, que o gzip não comprime) passam a ser
declarados pela primeira atribuição no escopo do arquivo (o mesmo `var`
içado). Fica a IIFE com diretivas ou `return` no topo.
`limitless_ui` −14 KB / −8,5 KB gzip; corpus −205 KB gzip.
`DARTFORGE_JSPROD_UNWRAP=0`.

### 15.6 Metadados das classes agrupados antes do `main` (`unwrap.rs`)

O `dart.lateFields` e as assinaturas de cada classe ficavam entre os corpos
de classe, onde o gzip não casava uma lista de nomes curtos com a seguinte.
Agora vão, na ordem, para logo antes da chamada do `main`: nada os lê
enquanto as bibliotecas carregam (as assinaturas são *thunks* preguiçosos,
os acessores `late` servem instâncias), menos as cópias feitas na carga
(`applyMixin`, `registerExtension`, `defineExtensionMethods`/`Accessors`,
`mixinOn`), cujas classes mantêm os seus. **−9,6 KB gzip** no
`limitless_ui`, −8,9 KB no `new_sali`. `DARTFORGE_JSPROD_GROUP_METADATA=0`.

Junto: as chaves dos campos `late` (sem os nomes, §13.5) saem como uma
*string* por classe (`"k1 k2"`), que o minificador reescreve palavra a
palavra depois do renomeio e ordena (`DARTFORGE_JSPROD_LATE_STRING=0`), e as
entradas do JSON das regras rti saem ordenadas pelo valor, com as *views*
vizinhas (`DARTFORGE_JSPROD_RULE_ORDER=0`).

### 15.7 *Tear-offs* com o tipo na chamada (`emit_js`, `sdk_patches.rs`)

O `dart.bind` lê o tipo da *tear-off* na tabela de assinaturas da classe,
então todo método destacado (cada `_handleEvent_N` das *views*) precisava de
uma entrada. Em produção, quando todo método de instância com aquele nome no
programa tem o mesmo tipo fechado e não genérico, e nenhum tem parâmetro
`covariant` explícito (cujos *stubs* de encaminhamento têm outro tipo), o
emissor escreve `dart.tbind(o, "m", T)`, um remendo do runtime que liga com o
tipo dado; o nome deixa de pedir assinatura (continua seletor vivo). O caso
do `corpus/js/264` (*stub* de covariância com tipo `(Object?, Object?)`)
pegou a primeira versão, sem a exclusão de `covariant`.
`DARTFORGE_JSPROD_TYPED_BIND=0`.

### 15.8 Argumentos finais sem uso descartados (`inline.rs`)

O *inline* recusava o `checkBinding(old, new, "expr", location)` das *views*
porque os argumentos são leituras de propriedade, e a expressão e a
localização (deduplicadas em variáveis `t$S`) também não eram descartáveis.
Uma chamada que não pode ser substituída perde agora os argumentos finais de
parâmetros que o corpo não lê, quando não podem lançar, e uma *string*
deduplicada (um `var` com valor literal, inicializado antes de tudo) conta
como tal. 3.370 chamadas no `limitless_ui`: **−78 KB brutos, −21,7 KB gzip**;
no `new_sali` −77 KB e −24,7 KB. `DARTFORGE_JSPROD_DROP_UNUSED_ARGS=0`.

### 15.9 Resultado

| alvo | início | agora | `dart2js -O4` | agora/`dart2js` |
| --- | ---: | ---: | ---: | ---: |
| `limitless_ui/example` | 3.610.318 (gz 1.036.655) | **3.455.727** (gz **969.872**) | 3.704.125 (gz 982.814) | 0,933× (gz 0,987×) |
| `new_sali/frontend` | 7.634.602 (gz 2.071.615) | **7.404.523** (gz **1.977.531**) | 7.614.136 (gz 2.018.523) | 0,972× (gz 0,980×) |
| corpus (273) | 18.852.978 (gz 6.198.985) | **17.242.249** (gz **5.572.874**) | 11.024.980 (gz 3.582.721) | 1,56× (gz 1,56×) |
| mediana nosso/`dart2js` | 1,574× (gz 1,621×) | **1,465×** (gz **1,474×**) | | |
| `print('hi')` | 47.555 (gz 16.431) | **44.383** (gz **14.959**) | 5.074 (gz 1.801) | 8,7× |
| `01_print` | 58.806 | 55.685 | 34.926 | 1,59× |
| `120_convert_json` | 163.921 | 97.136 | 58.550 | 1,66× |
| `135_convert_json_classes` | 238.599 | 178.342 | 101.193 | 1,76× |
| `136_string_interpolacao_avancada` | 212.238 | 149.892 | 90.463 | 1,66× |

Os dois projetos reais ficaram **menores que o `dart2js -O4` no bruto e no
gzip**. O corpus e os programas pequenos continuam acima.

### 15.10 Medidos e descartados

* Nomes de propriedade por posição dentro do grupo de coabitação (os campos
  de cada *view* com a mesma sequência de nomes): +1,8 KB gzip no
  `limitless_ui`; a ordem global por frequência é melhor.
* Tirar só o `toString` dos seletores vivos (limite superior, inseguro):
  nada muda no `print('hi')`; o piso é outro (abaixo).
* Tirar os três `findType` do fim do *bootstrap*: −30 bytes; o `_Universe`
  continua vivo por `instanceType`, `typeName` e as checagens do `JSArray`.
* `x == null` sem registrar o `==` resolvido: correto, mas sem efeito (o
  runtime usa `==` por outros caminhos).
* Constantes `static get x() { return null; }` (`_debugComponentUrl` das
  *views*) *inline*: só 114 restavam, −0,4 KB gzip.
* Tabela de nomes (`t$N`) em ordem de nome (ids reatribuídos): −0,7 KB
  gzip estimado; não feito (muda os ids de todas as receitas).

### 15.11 O que falta, por ganho esperado

1. **O piso** (`print('hi')` 44 KB contra 5 KB): mais da metade é o
   `dart:_rti` (`_Universe`, `_Parser`, subtipagem e `toString` de todo tipo),
   vivo porque o *bootstrap* e as classes nativas avaliam receitas
   (`instanceType(this)[_eval]` no `JSArray`, `findType`) e o `dart.str` do
   `print` chama `toString` sem tipo, o que acorda o `JSArray.toString` e o
   `StringBuffer`. O `dart2js` corta isso por inferência de tipo de argumento
   (o `print` só recebe `String`); aqui pede restringir o `toString` do
   `str`/`print` pelo tipo estático em cada chamada (delegação pelo
   parâmetro) e um caminho de receitas que só some quando nenhum tipo é
   avaliado.
2. **Registros no rti**: as funções de *record* do `dart:_rti`
   (`_lookupRecordRti`, `_isRecordSubtype`, `_recordRtiToString`,
   `createRecordTypePredicate`…) ficam vivas pelo `switch` de espécie mesmo
   sem *record* no programa; sairiam com uma condição constante "o programa
   tem receita de *record*".
3. **`unsafeCast` do ngdart**: o `@pragma('dart2js:as:trust')` faz o
   `dart2js` omitir o `as T` em qualquer `-O`; nós checamos (1.928 chamadas
   no `limitless_ui`). Honrar o *pragma* é mudança de contrato (decisão do
   dono).
4. Metadados que sobram no `limitless_ui` (gzip): assinaturas ~10,7 KB
   (`dload`/`dsend` dinâmicos dos *templates*), regras rti ~7,6 KB, tabela
   `t$N` ~7,6 KB, `lateFields` ~6,1 KB, recursos rti ~5 KB.

### 15.12 Observações da validação

* `corpus/modern` 340 e 341 falham no **desenvolvimento** (`Undefined name
  'cru'`, erro de linguagem do analisador antes da emissão) desde o começo da
  rodada; em produção passam. Nada desta rodada toca o analisador.
* O teste ignorado `tests/identity.rs` do `emit_js_production` (sem filtro e
  com `AllLive` emitem o mesmo texto) já falhava no `main` antes da rodada
  (`p3_classes.dart`).
* O `scripts/limitless-ui-production.sh` chama um `tool/serve_example.dart`
  que não existe mais no `limitless_ui`; o e2e foi rodado com um servidor
  estático (`python3 -m http.server`) no lugar dele.
* Suíte `tests/language` (`scripts/run-language-suite.py`): 2756/2863 iguais
  à VM, sem regressão (o emissor de desenvolvimento só mudou em §15.1; as
  demais mudanças do `emit_js` são do perfil de produção).

## 16. Rodada de 2026-10-27, tarde (macOS): o piso dos programas pequenos

Medido antes de mexer (o `main` em `b49226b2`, o fim da §15; macOS arm64,
Node 24.21, SDK 3.6.2, `gzip -9`; `dart2js -O4` do corpus refeito com o
mesmo SDK):

| alvo | nosso | `dart2js -O4` |
| --- | ---: | ---: |
| corpus (273 programas com saída do `dart2js`) | 17.242.249 (gz 5.572.874) | 11.024.980 (gz 3.582.721) |
| mediana nosso/`dart2js` | 1,465× (gz 1,474×) | |
| `print('hi')` | 44.383 (gz 14.956) | 5.074 (gz 1.801) |
| `01_print` | 55.685 (gz 18.224) | 34.926 (gz 11.543) |
| `limitless_ui/example` | 3.455.727 (gz 969.872) | 3.704.125 (gz 982.814) |
| `new_sali/frontend` | 7.404.523 (gz 1.977.531) | 7.614.136 (gz 2.018.523) |

Cada passo abaixo foi validado sozinho: corpus diferencial `corpus/js`
274/274 em desenvolvimento e produção e `corpus/modern` 26/26 em produção
(24/26 em desenvolvimento, as duas falhas antigas da §15.12), testes,
`clippy -D warnings` e `rustfmt` dos *crates* tocados, `limitless_ui` com o
e2e 15/15 e as 48 rotas sem erro no Chrome, `new_sali` carregando sem erro
(a mesma página de 6.087 caracteres). Cada otimização tem o seu
`DARTFORGE_JSPROD_*`.

### 16.1 Onde estava o piso

Duas ferramentas novas de diagnóstico:

* `DARTFORGE_JSPROD_MANGLE=0` (`minify.rs`) produz o arquivo final com todas
  as passagens menos o renomeio de identificadores; como o renomeio só troca
  nomes, os dois arquivos se alinham *token* a *token* e cada instrução do
  topo (`dart_rti$$_Universe`, `core$$RangeError`…) recebe os bytes que ocupa
  **no arquivo minificado de verdade**. O `dart2js` foi atribuído pelo
  *source map* (bytes por arquivo de origem);
* `DARTFORGE_JSPROD_SEL_WHY=nome` (`world`) imprime quem registra um seletor
  e com que cone; `DARTFORGE_JSPROD_RECORDS_WHY` o que torna os *records*
  vivos.

`01_print` (o `dart2js` tem 34.926 bytes: `rti.dart` 17,1 KB, código sem
mapa — *holders*, herança, tabelas — 8,8 KB, `js_helper` 4,6 KB e o resto
das bibliotecas 4,4 KB):

| parte | início | fim |
| --- | ---: | ---: |
| `dart:_rti` | 21.019 | 16.117 |
| runtime (`dart.*`) | 10.014 | 5.786 |
| `dart:core` | 6.005 | 4.123 |
| `dart:_interceptors` | 5.369 | 1.937 |
| `_js_helper` + `_internal` + `collection` | 5.789 | 2.313 |
| tabelas (`t$N`, `t$P`, `t$G`) | 3.340 | 1.866 |

O que inflava não era o `dart:_rti` em si (o do `dart2js` tem o mesmo
tamanho), e sim o que o mundo mantinha vivo **sem chamada**: ramos que o
emissor dobra e o mundo percorria, seletores universais registrados por
caminhos que o emissor escreve como operador do JavaScript, e o verificador
reavivando membros por nomes de *templates* `JS()` que falam de objetos do
JavaScript.

### 16.2 Operadores pelo tipo estático exato (`world`)

A resolução de um operador binário, de `x++` e de `x op= v` registrava o
operador de novo **sem cone**, então todo operador de `JSNumber`, `JSString`
e `JSArray` ficava vivo mesmo onde o emissor escreve o operador do
JavaScript. O alvo de `x++`/`x op= v` não tem tipo de leitura próprio; o tipo
do resultado (atribuído de volta) faz as vezes. A regra do emissor é
espelhada exatamente: interface `int`/`double`/`num` à esquerda (não uma
variável de tipo limitada por `num`, que o `18_math` pegou) e `String` não
anulável para `+`; o operador de um tipo de extensão continua pela resolução
(`230`). `DARTFORGE_JSPROD_OPERATOR_RESOLVED=0`. Corpus −303 KB.

### 16.3 `addRtiResources` só na cadeia de protótipos de uma instância

A receita no construtor e as marcas de tipo no protótipo só são lidas a
partir de objetos: classe que não é instanciada nem superclasse de uma
instanciada (`bool`, `String`, `int`, as classes de estáticos do rti,
interfaces) não precisa delas — quem implementa uma interface lista as marcas
dela. *Mixins*, `Object` e nativos com `registerExtension` mantêm (são
copiados na carga). `DARTFORGE_JSPROD_RTI_RESOURCES=0`. Corpus −380 KB.

### 16.4 *Inline* de `let v = A; return B;` e de corpo de uma instrução (`inline.rs`)

Os acessores do `dart:_rti` leem o campo num local e o devolvem por um
*cast* identidade; os *setters* e o `push` da pilha do *parser* são uma
instrução. Agora `let v = A; return B;` (com `B` lendo `v` uma vez, antes de
tudo) vira `B` com `(A)` no lugar, e uma instrução só é *inlined* onde o
valor da chamada é descartado. Nome nunca atribuído e sempre declarado com
valor é estável: pode ser argumento depois das leituras do corpo — provado
por *binding* com o `oxc_semantic` (o critério por nome perdia toda vez que
outra função reusava o nome). A estimativa de tamanho contava cada parêntese
que o *inline* acrescenta e o impressor tira. `DARTFORGE_JSPROD_INLINE_FORMS=0`,
`DARTFORGE_JSPROD_STABLE_NAMES=0`. Corpus −390 KB.

### 16.5 Sem *records*, sem os ramos de *record* do rti (`world`, `constants.rs`)

O mundo supõe que nenhum *record* existe: `k == Rti.kindRecord` (`k` local ou
um `_get…` do rti), `x is Record` e os `case` de `kindRecord`,
`Recipe.startRecord` e `_Parser.gotoRecord` dobram (no mundo e no emissor).
Um tipo ou literal de *record* no mundo, `RecordImpl` instanciado ou uma
receita de *record* no texto emitido recomeça o ponto fixo com os ramos.
`DARTFORGE_JSPROD_RECORDS_GATE=0`, `DARTFORGE_JSPROD_DEAD_CASES=0`.
Corpus −330 KB.

### 16.6 `JS_GET_FLAG` no mundo como no emissor

O emissor escrevia `JS_GET_FLAG('LEGACY')` e companhia como literal e
largava o ramo morto, mas o mundo percorria os dois: tudo o que só os
caminhos *legacy* e de checagens extras do `dart:_rti` chamam ficava vivo e
era emitido sem chamador. As `Constants` do mundo conhecem os mesmos valores
(DDC, *sound null safety*). **Corpus −660 KB**, `print('hi')` −2,5 KB.
`DARTFORGE_JSPROD_SDK_FLAGS=0`.

### 16.7 Nome que um *template* só chama não alcança *getter* (`world`, `verify.rs`)

Os *templates* do runtime chamam *builtins* de objetos JS (`#.entries()` e
`#.values()` do `Map` do `InternalMap`), e o mundo e o verificador tomavam o
nome como uso de todo membro Dart assim chamado: imprimir um mapa reavivava
`MapBase.entries` e com ele `MappedIterable`, `MapEntry` e os iteráveis de
mapa. Chamar um *getter* do JavaScript chamaria o seu valor; nome só visto
como `x.nome(` acorda e reaviva só métodos. O protocolo de iteração do
`for-in` passou a ser registrado no cone do iterável (`iterator`) e de
`Iterator` (`moveNext`, `current`), e um `JSArray` não registra nada (o
`for-of` do JavaScript o percorre). `01_print` 46,4 → 40,3 KB.
`DARTFORGE_JSPROD_TEMPLATE_CALLS=0`, `DARTFORGE_JSPROD_ITERATION_CONE=0`.

### 16.8 `hashCode` e `==` universais sem uso

* `definePrimitiveHashCode` (no epílogo de `JSNumber`/`JSString`/`JSBool`)
  copia o *getter* `hashCode` para o símbolo que só o `identityHashCode` do
  `dart:core` lê; emitido sempre, tornava `hashCode` seletor universal. Sai
  quando `identityHashCode` não vive. `DARTFORGE_JSPROD_PRIMITIVE_HASH=0`.
  Corpus −270 KB.
* `x == null` é o `== null` do JavaScript, e um `String`/número/`bool` não
  anulável à esquerda (ou como padrão constante, e as constantes de *enum*)
  compara por identidade: o emissor de produção escreve `===` em vez de
  `dart.equals`, o mundo não registra `==` nesses casos, e o
  `_installIdentityEquals` de `Object` só é emitido com `dart.equals` vivo.
  `DARTFORGE_JSPROD_PRIMITIVE_EQUALS=0`.

### 16.9 Os erros do JavaScript esperam uma exceção chegar ao Dart

`NativeError`, `JSNoSuchMethodError` e `JSRangeError` (marcas `Error`,
`TypeError`, `RangeError`) só aparecem quando uma exceção do JavaScript vira
valor Dart: um `catch` (`dart.getThrown`), um *template* cujo `catch` faz mais
que relançar (o construtor de `RegExp`) ou código estrangeiro. Até lá ficam
retidos como os nativos estrangeiros. Programas sem `catch` −1 KB.
`DARTFORGE_JSPROD_JS_ERRORS_GATE=0`.

### 16.10 Tipos *legacy*, `VARIANCE` e o resto do piso

* Sem tipos *legacy* (`T*`) em *sound null safety*: `k == Rti.kindStar`,
  comparações com `LEGACY_TYPE_REF<T>()` e os `case` de
  `kindStar`/`wrapStar` dobram, e o *bootstrap* só aquece `Object?`. Uma
  receita *legacy* no texto recomeça sem a suposição.
  `DARTFORGE_JSPROD_LEGACY_GATE=0`.
* `VARIANCE` é `false` em produção (nenhum parâmetro de tipo registra
  variância): `_areArgumentsSubtypes` fica com o caminho covariante.
  `DARTFORGE_JSPROD_VARIANCE=1` volta ao valor do DDC.
* Remendos do piso (`sdk_patches.rs`): o `defineLazyField` sem a contabilidade
  do *hot restart*, o `dart.global` sem ler `ddcSettings`, e o `_rtiToString`
  cortando o prefixo com `_Utils.substring` e crescendo/lendo o contexto
  genérico com `push`/índice do JavaScript (os caminhos checados de
  `String`/`List` traziam `RangeError`, `IndexError` e `UnsupportedError`).
* Variáveis de tipo de classe nas receitas e regras pelo id curto da classe
  (`|F.E` em vez de `JSArray.E`): o *parser* de receitas lê qualquer
  identificador com ponto como variável de tipo.
  `DARTFORGE_JSPROD_SHORT_TYPE_VARIABLES=0`.

### 16.11 Resultado

| alvo | início | agora | `dart2js -O4` | agora/`dart2js` |
| --- | ---: | ---: | ---: | ---: |
| corpus (273) | 17.242.249 (gz 5.572.874) | **13.731.358** (gz **4.500.777**) | 11.024.980 (gz 3.582.721) | 1,245× (gz 1,256×) |
| mediana nosso/`dart2js` | 1,465× (gz 1,474×) | **1,128×** (gz **1,143×**) | | |
| corpus sem os 25 programas de < 12 KB no `dart2js` | 16.125.676 | **12.941.694** | 10.891.833 | 1,188× (mediana 1,110×) |
| `print('hi')` | 44.383 (gz 14.956) | **31.277** (gz **10.756**) | 5.074 (gz 1.801) | 6,2× |
| `01_print` | 55.685 (gz 18.224) | **35.142** (gz **12.081**) | 34.926 (gz 11.543) | 1,006× (gz 1,047×) |
| `49_classes_heranca_super` | 55.269 | **42.026** | 37.618 | 1,117× |
| `120_convert_json` | 97.136 | **84.011** | 58.550 | 1,435× |
| `135_convert_json_classes` | 178.342 | **162.695** | 101.193 | 1,608× |
| `136_string_interpolacao_avancada` | 149.892 | **138.773** | 90.463 | 1,534× |
| `limitless_ui/example` | 3.455.727 (gz 969.872) | **3.435.623** (gz **965.530**) | 3.704.125 (gz 982.814) | 0,928× (gz 0,982×) |
| `new_sali/frontend` | 7.404.523 (gz 1.977.531) | **7.366.631** (gz **1.970.283**) | 7.614.136 (gz 2.018.523) | 0,967× (gz 0,976×) |

O corpus caiu 20% (−3,5 MB brutos, −1,07 MB gzip); 50 dos 273 programas
ficaram menores que o `dart2js -O4` no bruto (19 no gzip). Os dois projetos
continuam abaixo do `dart2js -O4` no bruto e no gzip. A meta de total e
mediana abaixo de 1,0× **não** foi alcançada (§16.13).

### 16.12 Medidos e descartados

* Tirar o aquecimento do *bootstrap* (`findType` de `Object*`, `Object?`,
  `Never*`): recursão infinita no *parser* de receitas (`Object?` precisa
  estar no *cache* antes do primeiro tipo anulável testado). Ficou só o de
  `Object?`, e os dois *legacy* saem com a §16.10.
* Podar as listas de `defineExtensionMethods`/`Accessors` pelos símbolos
  `dartx` usados: no `01_print` só 1 de 33 nomes não era usado.
* Lote dos `addRtiResources` do SDK (como o do programa, §12.7): ~6 bytes por
  instrução, ~150 bytes por programa contra o laço; não feito.
* Descrições curtas dos símbolos privados de armazenamento
  (`"ArgumentError.invalidValue"`): ~250 bytes por programa; não feito.

### 16.13 O que falta, por ganho esperado

1. **Os programas grandes do corpus** (JSON, `Uri`, `convert`, coleções:
   1,4–1,6×): a diferença é código de biblioteca. O `dart2js -O4` confia nos
   primitivos (sem checagem de índice nem de argumento: quase nenhum
   `RangeError`/`ArgumentError`/`IndexError`), não checa os *casts*
   implícitos e faz *inline* agressivo; nós mantemos as checagens (o
   diferencial compara com a VM). As regras rti também pesam (7,4 KB no
   `135`, com os nomes de variável de tipo do DDC por supertipo).
2. **`print('hi')` (31 KB contra 5 KB)**: o `dart2js` não emite o `dart:_rti`
   quando nenhum tipo é avaliado; aqui o rti (~16 KB) vive porque o
   *bootstrap*, as classes nativas (`JSArray.of`, os `runtimeType`) e o
   `getReifiedType` avaliam receitas, e o `toString` continua universal pelo
   `dart.str` do `print`. Restringir o `toString` pede inferir os tipos dos
   argumentos que chegam ao parâmetro do `print`/`str` (análise de fluxo à
   moda do `dart2js`), e tirar o rti pede que nenhuma receita seja avaliada
   — muitas mudanças pequenas nos caminhos do runtime. Os 25 programas
   minúsculos do corpus (790 KB contra 133 KB) são a maior parte do que
   separa o total da mediana.
3. **Chamadas dinâmicas** (`dsend`/`dload`, 39 programas): `_checkAndCall`,
   `InvocationImpl`, as assinaturas `t$G` e os mapas de argumentos nomeados
   (~7–10 KB por programa); o `dart2js` usa *stubs* de interceptor.
4. Os metadados por classe que o DDC exige (`addRtiResources`,
   `defineExtensionMethods/Accessors`, `t$N`, `t$P`): ~3,5 KB num programa
   médio.

### 16.14 Observações da validação

* Suíte `tests/language` (`scripts/run-language-suite.py`, desenvolvimento):
  2756/2863 iguais à VM, sem regressão; as mudanças do `emit_js` são todas
  do perfil de produção (filtro presente ou `Ctx::production`).
* O teste ignorado `tests/identity.rs` continua falhando como antes da
  rodada (`p3_classes.dart`, §15.12).
* Os testes ignorados `fresh_list_append` pegaram um remendo de substituição
  vazia (o `ddcSettings`): a prova de fonte canônica do *append* reverte cada
  remendo e precisa de um texto que apareça uma vez; corrigido antes do fim.
* Uma execução do e2e do `limitless_ui` falhou por tempo e passou 15/15 ao
  repetir com o mesmo arquivo (o mesmo *bundle* das execuções vizinhas).

## 17. Rodada de 2026-10-27, noite (macOS): metadados e declarações repetidas

Medido antes de mexer (o `main` em `ee0ed0aa`, o fim da §16; macOS arm64,
Node 24.21, SDK 3.6.2, `gzip -9`; mesmos `dart2js -O4` da §16):

| alvo | nosso | `dart2js -O4` |
| --- | ---: | ---: |
| corpus (273 programas com saída do `dart2js`) | 13.731.358 (gz 4.500.777) | 11.024.980 (gz 3.582.721) |
| mediana nosso/`dart2js` | 1,128× (gz 1,143×) | |
| `print('hi')` | 31.277 (gz 10.756) | 5.074 (gz 1.801) |
| `01_print` | 35.142 (gz 12.081) | 34.926 (gz 11.543) |
| `limitless_ui/example` | 3.435.623 (gz 965.530) | 3.704.125 (gz 982.814) |
| `new_sali/frontend` | 7.366.631 (gz 1.970.283) | 7.614.136 (gz 2.018.523) |

Cada passo abaixo foi validado sozinho: corpus diferencial `corpus/js`
274/274 em desenvolvimento e produção e `corpus/modern` 26/26 em produção
(24/26 em desenvolvimento, as duas falhas antigas da §15.12), testes
(inclusive os ignorados) dos *crates* tocados, `clippy -D warnings` e
`rustfmt`, `limitless_ui` com o e2e 15/15 e as 48 rotas sem erro no Chrome,
`new_sali` carregando sem erro (a mesma página de 6.087 caracteres). Cada
otimização tem o seu `DARTFORGE_JSPROD_*=0`. A atribuição de bytes usou o
`DARTFORGE_JSPROD_MANGLE=0` da §16.1 sobre o corpus inteiro (bytes do arquivo
minificado por instrução do topo) e o *source map* do `dart2js`.

### 17.1 Assinaturas que nenhuma leitura alcança (`emit_js`, `own_sdk.rs`)

* A entrada `[dartx.nome]` que o DDC duplica nas assinaturas de uma classe
  do SDK que não é nativa só é lida por uma *tear-off* pelo símbolo
  (`dart.bind(o, $nome)`): o despacho dinâmico chega a um objeto Dart pela
  *string* (`_canonicalMember` só troca pelo símbolo num nativo). Em produção
  a duplicata fica só para as classes da cadeia de protótipos de um nativo
  (`Ctx::in_native_chain`: o nativo, as superclasses e os *mixins*, cuja
  tabela o `registerExtension` entrega ao tipo JavaScript) e para os nomes
  destacados pelo símbolo. `DARTFORGE_JSPROD_SYMBOL_SIGNATURES=0`.
* As assinaturas dos apelidos de interface nativa herdados de *mixin*
  (`inherited_native_method_signatures`) eram emitidas mesmo quando nenhum
  `dsend`/`dload`/`bind` lia o nome; agora seguem a regra das outras
  entradas. `DARTFORGE_JSPROD_ALIAS_SIGNATURES=0`. No `122` sobra só a
  assinatura de `>` do `JSNumber` (a única chamada dinâmica do programa).
* **Defeito corrigido**: a extração dos nomes das assinaturas não lia o
  segundo argumento de `dart.bind(o, $add)` (um apelido de `dartx`, não uma
  *string*): a *tear-off* `l.add` de uma classe do SDK ou do programa que
  implementa `List` perdia o tipo reificado (`runtimeType` dava
  `JavaScriptFunction` e uma chamada dinâmica não checava o argumento).
  Teste `signature_tearoffs`.

Corpus **−334 KB** (13.731.358 → 13.397.415; gz −95 KB).

### 17.2 Nomes privados do runtime agrupados por biblioteca (`private_names.rs`)

Cada símbolo privado do SDK era uma declaração `x = dart$$privateName(L,
"_n")`, minificada em `ie=v(h,`_n`),`. Depois da compressão do `oxc` (que já
tirou os não lidos), uma sequência dessas declarações no topo vira uma
desestruturação por biblioteca, `[a, b] = t$V(L, "_a _b")`, com as mesmas
chamadas na mesma ordem de biblioteca; um par repetido vira apelido do
primeiro (o `privateName` guarda um símbolo por biblioteca e nome). As
descrições continuam reservadas como nomes de propriedade.
`DARTFORGE_JSPROD_GROUP_PRIVATE_NAMES=0`. Corpus −108 KB; o gzip fica +3 KB
(com os mesmos nomes, a forma agrupada custa ~30 bytes gzip por programa:
o gzip já comprimia bem o `=v(h,` repetido). Feito antes da compressão, como
passo de texto, perdia o que a compressão remove depois (no `240`, −5 bytes).

### 17.3 Ids curtos de receita pelo mundo fechado (`filter.rs`, `emit_js`)

Os ids curtos das receitas (§9.5) eram da ordem do programa inteiro: as
classes mortas ocupavam ids curtos e deixavam buracos na tabela `t$N`. Agora
cada rodada do ponto fixo ordena (`recipe_ids_ranked`): as classes vivas das
frequentes (`int`, `String`, `List`…), as instanciadas, as só usadas como
tipo, e as mortas no fim. O emissor, o verificador e a tabela de nomes leem
o mesmo vetor (`WorldReport::recipe_ids`). `DARTFORGE_JSPROD_RANKED_IDS=0`.
Corpus −136 KB (gz −15 KB); `print('hi')` −500 bytes.

Medidos e descartados: ordenar pela contagem de citações da rodada anterior
(corpus −8 KB a mais, mas os dois projetos +2 a +4 KB gzip) e só pelo nível
(as frequentes do tipo `int` perdiam o id de uma letra: `new_sali` +3,9 KB).

### 17.4 *Setter* de encaminhamento de campo `final` (`emit_js`, `module.rs`)

Um campo `final` virtual ganhava `set x(v) { super.x = v; }`, que só existe
para manter alcançável um *setter* herdado. Sem *setter* em superclasse ou
*mixin* ninguém o chama (o Dart não tem *setter* a chamar, e um `dput` não
acha assinatura de *setter*). `DARTFORGE_JSPROD_FINAL_SETTERS=0`. Corpus
−97 KB (gz −20 KB). Teste `final_field_setters` (o *setter* herdado de
superclasse e de *mixin* continua funcionando; a escrita dinâmica continua
lançando `NoSuchMethodError`).

### 17.5 `Object.hash` por aridade (`sdk_patches.rs`, `world`, `emit_js`)

`Object.hash` despacha por sentinelas nos opcionais e mantinha vivas as 19
aridades e todos os `SystemHash.hashN` (~5 KB em 25 programas; o
`DateTime.hashCode` do SDK usa `Object.hash`). Um remendo de produção dá a
`dart:core` as funções `_hash2` … `_hash20`, cada uma o corpo que
`Object.hash` roda com aquele número de argumentos, e o emissor e o mundo
mandam a chamada estática com `n` posicionais para `_hashN`
(`arity_specialization`, a mesma regra nos dois; uma divergência o
verificador corrige). A *tear-off* continua indo ao original.
`DARTFORGE_JSPROD_HASH_ARITY=0`. Corpus −50 KB. Teste `object_hash_arity`
(cada aridade dá o mesmo *hash* que o corpo original pela *tear-off*).

### 17.6 Recursos rti e apelidos de extensão em lote (`rti_rules.rs`, `unwrap.rs`)

* Os `addRtiResources` do SDK entram no lote do programa (§12.7), menos os
  das classes cujas marcas uma chamada do runtime copia na carga: argumentos
  de `applyMixin`/`registerExtension`, classes com `[dart.mixinOn]` e todas
  as superclasses delas (o `registerExtension` copia as marcas da cadeia
  Dart inteira de um nativo). Sem superclasse legível de uma delas, o SDK
  fica como estava (é o que acontece no `new_sali`).
  `DARTFORGE_JSPROD_BATCH_SDK_RESOURCES=0`. Corpus −31 KB.
* Os `defineExtensionMethods`/`Accessors` viram uma chamada só, com lista
  plana (classe, métodos, acessores), depois dos metadados agrupados antes do
  `main` (então todo acessor `late` já existe), com as mesmas exclusões. As
  *strings* do lote continuam reservadas (`t$X` entra em
  `call_takes_names`). Abaixo de 28 instruções o lote perde.
  `DARTFORGE_JSPROD_BATCH_EXTENSIONS=0`. Corpus −18 KB (gz −7 KB).

### 17.7 Símbolos `dartx` e seus apelidos numa desestruturação (`private_names.rs`)

O *bootstrap* cria cada símbolo `dartx` num laço e o topo declara os
apelidos `$n = dartx.n` (§15.4). Depois da compressão, os dois viram uma
declaração no lugar do laço: `var [$a, , $c] = t$Y(dartx, "a b c")`, que cria
os símbolos na mesma ordem e os devolve. Os apelidos são `var` do mesmo
escopo, só inicializados mais cedo (antes de toda classe). Um segundo
apelido do mesmo nome fica como estava.
`DARTFORGE_JSPROD_GROUP_EXTENSION_SYMBOLS=0`. Corpus **−74 KB, gz −23 KB**.

### 17.8 Resultado

| alvo | início | agora | `dart2js -O4` | agora/`dart2js` |
| --- | ---: | ---: | ---: | ---: |
| corpus (273) | 13.731.358 (gz 4.500.777) | **12.882.590** (gz **4.330.102**) | 11.024.980 (gz 3.582.721) | 1,168× (gz 1,209×) |
| mediana nosso/`dart2js` | 1,128× (gz 1,143×) | **1,071×** (gz **1,112×**) | | |
| corpus sem os 25 programas de < 12 KB no `dart2js` | 12.941.694 | **12.127.128** | 10.891.833 | 1,113× (mediana 1,050×) |
| `print('hi')` | 31.277 (gz 10.756) | **29.920** (gz **10.532**) | 5.074 (gz 1.801) | 5,9× |
| `01_print` | 35.142 (gz 12.081) | **33.595** (gz **11.770**) | 34.926 (gz 11.543) | **0,962×** (gz 1,020×) |
| `48_classes_equals_hashcode` | 53.551 | **45.123** | 40.637 | 1,110× |
| `63_iterable_expand_reduce_lazy` | 97.210 | **85.011** | 60.528 | 1,404× |
| `122_collection_queue` | 90.645 | **81.980** | 53.242 | 1,540× |
| `135_convert_json_classes` | 162.695 | **145.622** | 101.193 | 1,439× |
| `limitless_ui/example` | 3.435.623 (gz 965.530) | **3.403.095** (gz **958.730**) | 3.704.125 (gz 982.814) | 0,919× (gz 0,975×) |
| `new_sali/frontend` | 7.366.631 (gz 1.970.283) | **7.301.713** (gz **1.955.451**) | 7.614.136 (gz 2.018.523) | 0,959× (gz 0,969×) |

O corpus caiu 6,2% (−849 KB brutos, −171 KB gzip); 87 dos 273 programas
estão abaixo do `dart2js -O4` no bruto (eram 50) e 26 no gzip (eram 19). Os
dois projetos continuam abaixo do `dart2js -O4` no bruto e no gzip. A meta
de total e mediana abaixo de 1,0× **não** foi alcançada (§17.10).

### 17.9 Medidos e descartados

* `print(s)` com `String` não anulável direto para uma `_printLine` (remendo
  do SDK, mundo e emissor espelhados): `print('hi')` −7 bytes — o rti
  continua vivo pelo *bootstrap* (`findType`, `addRules`), pelos nativos
  (raízes do `registerExtension`), pelo `getReifiedType` do
  `Object.toString` e pelos caminhos de erro (`str` → `ArgumentError` →
  `Error.safeToString` → `JSArray.toString`…); o corpus ficava +5 KB
  (`print` e `_printLine` vivos juntos). Revertido.
* Podar as marcas de `addRtiResources` de tipos que nenhum teste pode ver:
  ~20 entradas extras num programa médio (~100 bytes). Não feito.
* Podar as listas de `defineExtension*` pelos nomes `dartx` usados: 193 de
  18.114 nomes do corpus sem uso. Não feito.
* `List.sort` pelo `sort` do JavaScript (o que o `dart2js` faz): muda a
  ordem dos elementos iguais em relação à VM, que usa o mesmo `Sort` do
  `dart:_internal` que nós. Não pode.
* Descrições curtas dos símbolos de armazenamento (`"ArgumentError.message"`):
  aparecem nas mensagens de erro do JavaScript que o `JSNoSuchMethodError`
  converte. Não feito.

### 17.10 O que falta, por ganho esperado

1. **Os 25 programas minúsculos** (755 KB contra 133 KB): o rti (~16 KB) e
   a instalação das classes nativas. O experimento do `print` (§17.9)
   confirmou que nenhuma mudança isolada basta: é preciso o *bootstrap* rti
   só com o rti vivo, `is num/int/double` como `typeof` (hoje só `String` e
   `bool`), o `strSafe` sem o `ArgumentError` (impossível em *null safety*
   sólido para um `toString` Dart) e os nativos instalados sob demanda.
2. **Chamadas dinâmicas** (`dload`/`dsend`): `_checkAndCall`,
   `_argumentErrors`, `InvocationImpl`, `NoSuchMethodError`, ~4–5 KB por
   programa que tem uma; o `dart2js` usa *stubs* de interceptor.
3. **Biblioteca nos programas de 1,3–1,5×**: `Uri` (~18 KB contra 12–13 KB
   no `dart2js`), os iteráveis de *cast* que o `Iterable.generate` sem
   gerador mantém, `ListQueue`/`DoubleLinkedQueue`. O `Sort` fica (paridade
   com a VM).
4. O lote do SDK (§17.6) desiste no `new_sali` (uma superclasse de classe
   copiada não é legível no texto); ler a hierarquia pelo modelo em vez do
   texto o destravaria.

### 17.11 Observações da validação

* As falhas intermitentes do e2e do `limitless_ui` (§16.14) são do servidor
  de teste: `console.error: Failed to load resource:
  net::ERR_CONNECTION_RESET` do `python3 -m http.server` numa navegação; o
  mesmo *bundle* passa 15/15 ao repetir.
* Os testes ignorados `fresh_list_append` precisam de `DARTFORGE_SDK_LIB`
  apontando para o `lib/` do SDK.
* O teste ignorado `tests/identity.rs` continua falhando como antes
  (`p3_classes.dart`, §15.12).
* Suíte `tests/language` (desenvolvimento, CLI compilado à parte em
  `target/r17-cli`): 2756/2863 iguais à VM, sem regressão; as mudanças do
  `emit_js` são todas do perfil de produção.

## 18. Rodada de 2026-10-27, madrugada (macOS): o piso dos programas minúsculos

Medido antes de mexer (o `main` em `be52b698`, o fim da §17; macOS arm64,
Node 24.21, SDK 3.6.2, `gzip -9`; os mesmos `dart2js -O4` da §16):

| alvo | nosso | `dart2js -O4` |
| --- | ---: | ---: |
| corpus (273 programas com saída do `dart2js`) | 12.882.590 (gz 4.330.102) | 11.024.980 (gz 3.582.721) |
| mediana nosso/`dart2js` | 1,071× (gz 1,112×) | |
| os 25 programas de < 12 KB no `dart2js` | ~755 KB | 133.147 |
| `print('hi')` | 29.920 (gz 10.532) | 5.074 (gz 1.801) |
| `void main() {}` | 29.588 | |
| `01_print` | 33.595 (gz 11.770) | 34.926 (gz 11.543) |
| `limitless_ui/example` | 3.403.095 (gz 958.730) | 3.704.125 (gz 982.814) |
| `new_sali/frontend` | 7.301.713 (gz 1.955.451) | 7.614.136 (gz 2.018.523) |

Cada passo abaixo foi validado sozinho: corpus diferencial `corpus/js`
274/274 em desenvolvimento e produção e `corpus/modern` 26/26 em produção
(24/26 em desenvolvimento, as duas falhas antigas da §15.12), testes
(inclusive os ignorados) de `world`, `emit_js` e `emit_js_production`,
`clippy -D warnings` e `rustfmt`, `limitless_ui` com o e2e 15/15 e as 48
rotas sem erro no Chrome e `new_sali` carregando sem erro (a mesma página de
6.087 caracteres). Os quatro primeiros passos foram validados também em
conjunto, desligando pelas chaves os que vêm depois de cada um (os estados
intermediários dos *commits*). Cada otimização tem o seu
`DARTFORGE_JSPROD_*=0`.

### 18.1 Por que nenhuma mudança isolada bastava

Até um `void main() {}` tinha 29,6 KB: o mundo fechado não mantinha o rti
vivo, quem o mantinha era o texto que o emissor escrevia sem perguntar e que
o verificador então reenraizava. Na primeira rodada do ponto fixo de um
programa vazio o texto já citava:

* as regras rti (`_Universe.addRules`), o aquecimento do universo
  (`findType("|a?")`) e os `addRtiResources` de toda classe;
* o construtor de `Object`, uma guarda que só dispara num `new` JavaScript de
  uma classe Dart e nomeava a classe com `typeName(getReifiedType(this))`.

Num programa que imprime, o `print` convertia o argumento com o `dart.str`
genérico: `toString` universal, `JSArray.toString` (`StringBuffer`),
`Object.toString` (`getReifiedType`, o rti), e o `ArgumentError` de "toString
devolveu null". A interpolação de um `int` ia pelo `strSafe`, com a mesma
chamada de extensão e o mesmo `ArgumentError`. Cada uma dessas vias sozinha
mantinha tudo: desligar qualquer uma das quatro primeiras mudanças devolve o
corpus a ~12,87 MB (as medidas por chave estão em 18.8).

Ferramenta nova de diagnóstico: `DARTFORGE_JSPROD_SEL_WHY=*` imprime todo
seletor registrado, com o cone e quem o registrou (função ou variável).

### 18.2 O *bootstrap* rti só com o rti vivo (`emit_js`, `filter.rs`)

`Liveness::rti_live`: o rti pode rodar quando alguma função do `dart:_rti`, ou
uma função do runtime que lê os recursos rti de uma classe (`isDartClass`,
`getClassName`), está viva no mundo da rodada. Sem isso não se emitem as
regras, o `findType` de aquecimento nem nenhum `addRtiResources` (só o rti
lê o que eles escrevem). O predicado é monótono no ponto fixo: o que o
*bootstrap* cita quando ele vale mantém o rti vivo.
`DARTFORGE_JSPROD_RTI_GATE=0`.

A guarda do construtor de `Object`, em produção, lança sem nomear a classe:
o código compilado nunca faz `new` de uma classe Dart (chama `C.new`), e ler
`constructor.name` tornaria `name` um seletor vivo.
`DARTFORGE_JSPROD_OBJECT_GUARD=0`.

`void main() {}`: 29.588 → **2.129** bytes. Sozinho no corpus, −4 KB (os
programas que imprimem ainda chegam ao rti pelo `print`).

### 18.3 `print` e `toString` de primitivos convertidos no lugar (`sdk_patches.rs`, `world`, `emit_js`)

* Um remendo do SDK divide o `print`: `print(object) =>
  _printLine("$object")` e `_printLine(line)`. A chamada `print(x)` vira
  `_printLine` com `x` convertido como numa interpolação, pelo tipo
  estático; o mundo usa `_printLine` também (o `print` fica para
  *tear-offs*). `DARTFORGE_JSPROD_PRINT_LINE=0`.
* Outro remendo dá ao runtime o `strNum`, o próprio `JSNumber.toString`. Um
  `int`/`double`/`num` não anulável interpola por ele; um `bool` não anulável
  pelo `+` do JavaScript (o mesmo `true`/`false` do `JSBool.toString`); e
  `x.toString()` num `String`, número ou `bool` não anulável é essa mesma
  conversão, sem registrar `toString` no mundo.
  `DARTFORGE_JSPROD_STR_PRIMITIVES=0`.

Com a §18.2: `print('hi')` 29.920 → **2.443** bytes (gz 1.130); corpus
12.878.565 → 12.030.063.

### 18.4 Módulo e deslocamentos por literal (`emit_js`, `world`)

* `i % k` com `int` não anulável à esquerda e literal decimal de 1 a 2^31−1 à
  direita é `(i % k + k) % k`: exato para inteiros, nunca `-0`, o valor do
  `JSNumber.%` (enquanto o corpo dele for certificado como o módulo
  euclidiano canônico, a mesma condição dos contadores), sem a chamada e as
  suas checagens de argumento. `DARTFORGE_JSPROD_LITERAL_MODULO=0`. Corpus
  −30 KB; `21_while_do_while` 31.190 → 3.266.
* `i << k`, `i >> k`, `i >>> k` com `int` não anulável e contagem literal não
  negativa: o corpo do operador com a contagem conhecida (`(i << k) >>> 0`,
  `i >>> k`, e para `>>` até 31 um ajudante do runtime remendado,
  `dart.shrCount`, porque o receptor é lido duas vezes; acima de 31 a forma
  em linha que o `JSNumber` dá, ainda avaliando `i`). O `ArgumentError` de
  contagem negativa não pode acontecer. O mundo não registra o operador, nem
  `unary-`/`~` de um literal inteiro (o literal não tem tipo registrado).
  `DARTFORGE_JSPROD_LITERAL_SHIFTS=0`. `146_antigo_colecoes_bits` 30.532 →
  2.857; corpus −51 KB. A primeira versão escrevia o `>>` como condicional
  com temporário em linha: o `new_sali` (cheio de deslocamentos em código de
  criptografia e PDF) crescia 6 KB; o ajudante o deixa 2 KB menor.

### 18.5 Lista literal de `for-in` como *array* nu (`emit_js`, `world`)

`for (x in [a, b])` montava a lista com `JSArray.of` e o tipo reificado, que
ninguém observa: a lista não escapa do laço e o `for-of` do JavaScript
percorre o mesmo *array*. Em produção sai `[a, b]`, e o mundo não registra o
protocolo de iteração para uma lista literal (é um `JSArray` qualquer que
seja o tipo estático). `DARTFORGE_JSPROD_BARE_LOOP_LISTS=0`.
`23_break_continue_rotulos` 30.992 → 3.556; corpus −28 KB.

### 18.6 O lote dos recursos rti do SDK no `new_sali` (`rti_rules.rs`)

O lote da §17.6 desistia do SDK inteiro quando a superclasse de uma classe
copiada (`applyMixin`/`registerExtension`) não era legível no texto do SDK:
no `new_sali` era uma aplicação de *mixin* do programa (`const
FilteredElementList$mixin0 = class … extends core.Iterable {};`). As linhas de
superclasse agora são lidas do arquivo inteiro; um nome local declarado em
duas bibliotecas (`XmlStartElementEvent$mixin0`) segue todas as
superclasses que recebe (fixar a mais só custa bytes).
`DARTFORGE_JSPROD_TRACE` nomeia a classe quando o lote ainda desiste;
`DARTFORGE_JSPROD_BATCH_PROGRAM_SUPERS=0` volta a ler só o SDK.

Ler a hierarquia pelo modelo, como sugerido, não resolveria: as aplicações
de *mixin* (`X$mixin0`) são criadas pelo emissor e não existem no modelo de
elementos; o texto é a fonte que as contém. `new_sali` −2,9 KB (gz −1,1
KB), `limitless_ui` −2 KB.

### 18.7 Defeito corrigido: regras rti idênticas por apelido, não por *string*

O encaminhamento de regras idênticas (`"|B": "|A"`, rodadas anteriores)
mudava a semântica: o rti lê uma regra *string* como "`B` é `A`"
(`_isInterfaceSubtype` troca o nome da classe e continua), então duas
classes sem relação com os mesmos supertipos viravam subtipo uma da outra:
`<B>[] is List<A>` dava `true` em produção (a VM dá `false`). O corpo repetido
agora sai do JSON e a classe recebe o **mesmo objeto** de regra depois da
última chamada de `addRules` (`tR["|B"] = tR["|A"]`, num laço sobre os
pares): as mesmas consultas de supertipo e substituição, e `B` continua
uma classe à parte. A fonte também precisa ser registrada uma vez só,
chaves que o `addOrUpdateRules` toca (ele altera a regra no lugar) nunca
viram apelido, e abaixo do custo do laço todo corpo fica. Teste
`shared_rule_bodies` (as checagens de subtipo contra a VM, com e sem
`DARTFORGE_JSPROD_RULE_FORWARDING`). Corpus +13 KB (os poucos apelidos de um
programa pequeno não pagam o laço); `limitless_ui` −2,7 KB, `new_sali`
−4,4 KB.

### 18.8 Medidas por chave (corpus no estado da §18.5)

| desligado | corpus |
| --- | ---: |
| nada | 11.971.350 |
| `DARTFORGE_JSPROD_RTI_GATE=0` | 12.874.699 |
| `DARTFORGE_JSPROD_OBJECT_GUARD=0` | 12.878.722 |
| `DARTFORGE_JSPROD_PRINT_LINE=0` | 12.860.574 |
| `DARTFORGE_JSPROD_STR_PRIMITIVES=0` | 12.833.343 |
| `DARTFORGE_JSPROD_LITERAL_MODULO=0` | 12.001.990 |
| `DARTFORGE_JSPROD_BARE_LOOP_LISTS=0` | 11.999.423 |

Os quatro primeiros são uma coisa só: sem qualquer um deles o rti volta.

### 18.9 Resultado

| alvo | início | agora | `dart2js -O4` | agora/`dart2js` |
| --- | ---: | ---: | ---: | ---: |
| `print('hi')` | 29.920 (gz 10.532) | **2.443** (gz **1.130**) | 5.074 (gz 1.801) | **0,48×** (gz 0,63×) |
| `void main() {}` | 29.588 | **2.129** | | |
| `01_print` | 33.595 (gz 11.770) | **33.564** (gz **11.750**) | 34.926 (gz 11.543) | 0,961× (gz 1,018×) |
| os 25 programas de < 12 KB no `dart2js` | ~755 KB | **96.286** (gz 40.796) | 133.147 (gz 47.898) | **0,723×** (gz 0,852×) |
| corpus (273) | 12.882.590 (gz 4.330.102) | **11.929.493** (gz **4.006.053**) | 11.024.980 (gz 3.582.721) | 1,082× (gz 1,118×) |
| mediana nosso/`dart2js` | 1,071× (gz 1,112×) | **1,036×** (gz **1,083×**) | | |
| corpus sem os 25 minúsculos | 12.127.128 | **11.833.207** | 10.891.833 | 1,086× (mediana 1,052×) |
| `limitless_ui/example` | 3.403.095 (gz 958.730) | **3.397.391** (gz **958.177**) | 3.704.125 (gz 982.814) | 0,917× (gz 0,975×) |
| `new_sali/frontend` | 7.301.713 (gz 1.955.451) | **7.292.089** (gz **1.954.844**) | 7.614.136 (gz 2.018.523) | 0,958× (gz 0,968×) |

O corpus caiu 7,4% (−953 KB brutos, −324 KB gzip); 113 dos 273 programas
estão abaixo do `dart2js -O4` no bruto (eram 87) e 59 no gzip (eram 26). Dos
25 programas minúsculos, 24 ficaram menores que o `dart2js` (2,5–3,6 KB
contra 5,1–6,3 KB); 35 dos 274 programas do corpus não carregam mais o
rti. Os dois projetos continuam abaixo do `dart2js -O4` no bruto e no gzip.
A meta de total e mediana abaixo de 1,0× **não** foi alcançada (§18.11).

### 18.10 Medidos e descartados

* `toString` registrado pelo cone de quem chega às conversões do runtime
  (interpolação, `print`, `throw`) em vez de universal: nenhum byte no
  corpus. Mesmo cortando por completo o registro universal (limite
  superior, inseguro) nada muda: o `toString` também é alcançado pelos
  *templates* `JS()` que chamam `#.toString()` sobre valores do JavaScript,
  que valem para toda classe que não é nativa.
* O `>>` por literal escrito em linha com temporário (antes do ajudante
  `shrCount`): `new_sali` +6 KB (18.4).

### 18.11 O que falta, por ganho esperado

1. **Os programas médios (1,3–1,6×)**: a diferença é código de biblioteca
   com as checagens que mantemos (`Uri` ~19 KB contra 13 KB no `139`; Base64
   e UTF-8 ~8 KB contra 3,5 KB; filas, mapas e iteráveis). O `dart2js -O4`
   confia nos primitivos e faz *inline* agressivo; nós comparamos com a VM.
2. **Chamadas dinâmicas** (46 programas): `_checkAndCall`,
   `_argumentErrors`, `InvocationImpl` e o caminho de argumentos nomeados
   (`LinkedMap`, `UnmodifiableMapView`, `MapBase`, os iteráveis do `where`)
   somam ~4–5 KB. Podar pelos formatos vivos pede estender a análise de
   parâmetros sempre nulos (`params.rs`, hoje só fora do SDK) às funções do
   runtime, com as aridades das chamadas que o texto escreve (`dsend` sem
   nomeados ⇒ `named` nulo até o `InvocationImpl`). Não feito.
3. **`20_if_else_encadeado`** (31,6 KB contra 6,4 KB), o único minúsculo que
   sobrou: `String.startsWith` tem o caminho de `RangeError` (índice fora do
   intervalo), que mantém o rti pelo `toString` dos erros. Com índice 0 e
   padrão `String` ele não pode falhar; o `startsWith`/`endsWith`/`contains`
   do JavaScript dariam o mesmo resultado, mas é mais um caso a espelhar no
   mundo.
4. **Regras rti**: ~308 KB de JSON no corpus, ~104 KB deles substituições de
   variáveis de tipo (`"|w.E":"|u"`) que repetem a lista do supertipo
   (`"|w":["|u"]`). Nomear as variáveis pela posição (`|w.0`) e derivá-las no
   `evalTypeVariable` tiraria essas entradas.

### 18.12 Observações da validação

* O teste ignorado `tests/identity.rs` continua falhando como antes
  (`p3_classes.dart`, §15.12).
* Uma execução das rotas do `limitless_ui` acusou um erro de carga do
  `quill_table_better.js` (terceiro, lê `Quill.import` antes do `Quill`
  existir); o mesmo *bundle* passou 48/48 ao repetir.
* O compilador de desenvolvimento usado no diferencial foi construído à
  parte (`target/r18-cli`): o `emit_native`/`runtime` de outro agente
  estavam sendo editados na mesma árvore.
* Suíte `tests/language` (desenvolvimento, com esse CLI): 2756/2863 iguais à
  VM, sem regressão; as mudanças do `emit_js` são todas do perfil de
  produção (filtro presente, `Ctx::production` ou remendos do SDK, que só o
  perfil de produção compila).

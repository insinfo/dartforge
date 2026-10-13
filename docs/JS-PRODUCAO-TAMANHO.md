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

1. `dartforge-differential --producao`: corpus/js 238/238 iguais à VM.
2. Testes de `mundo`, `emit_js`, `emit_js_production` e `elements`.
3. `limitless_ui/example`: e2e 26/26.
4. `new_sali/frontend`: `scripts/fluxo.mjs`, os 11 passos sem erro.
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

* corpus/js `--producao` 238/238;
* testes dos crates verdes;
* `limitless_ui` 26/26 no e2e;
* `new_sali` com o `fluxo.mjs` igual ao do bundle do `dart2js`: 11
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
* `new_sali` com o `fluxo.mjs` igual ao controle `dart2js`.

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

* cada mudança tem a sua spec curta e é validada com o corpus `--producao`,
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

`DARTFORGE_JSPROD_REGRAS=0` desliga.

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
`DARTFORGE_JSPROD_NOMEADOS=0` desliga.

| projeto | antes | depois |
| --- | ---: | ---: |
| `new_sali/frontend` | 12.601.153 | **12.356.255** |
| `limitless_ui/example` | 7.073.375 | **6.942.798** |
| `01_print` | 157.690 | **156.012** |

Validação: corpus 238/238 (entre eles os quatro com `Function.apply`),
`limitless_ui` 26/26, `new_sali` igual ao controle.

### 8.3 `--omitir-checagens` (o `-O4` do dart2js), opcional e inseguro

O padrão continua com a semântica da VM, como o `dart2js -O2`. Com
`--omitir-checagens` (ou `-O4`) saem as conversões implícitas de `dynamic`
(`FnEmitter::coerce_to`) e as checagens de covariância de parâmetro
(`sdk_proprio::verificacoes_de_covariancia`), como o `omitImplicitChecks`
do `dart2js` (`options.dart:1102-1126`). Um programa com erro de tipo
segue adiante em vez de lançar `TypeError`. A comparação justa com o
`dart2js -O4` é com a opção ligada.

Medido no `new_sali`:

| configuração | bytes |
| --- | ---: |
| padrão (semântica da VM) | 12.356.255 |
| `--omitir-checagens` | 12.286.128 |
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

Validação (2026-10-06): corpus `--producao` 238/238, `limitless_ui` 26/26 no
e2e, `new_sali` com o `fluxo.mjs` nos 11 passos sem erro, igual ao controle
sem o passo.

## 9. Rodada de 2026-10-12 (macOS): rumo a menos que o `dart2js -O4`

Meta do dono: arquivo **menor** que o do `dart compile js -O4`, com o corpus
de produção 238/238 igual à VM. Medido antes de mexer
(`scripts/medir-js-producao-tokens.py`, categorias de token do arquivo
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
  `"dartforge:fim-do-sdk"` que `proprio::montar` põe depois do SDK)
  continuam todas reservadas;
* no programa, só as que estão nos argumentos de uma chamada `dart.X` (fora
  das que recebem mensagens, constantes e descrições: `privateName`,
  `const*`, `nullCheck`, `throw`, `fn`, `str`, `lateFields`,
  `recordLiteral`…) ou `js_util.X`, à esquerda de `in`, e nos argumentos de
  `defineProperty`/`hasOwnProperty`/`Reflect.get`… O contexto vale até a
  próxima chamada, `new` ou corpo de função com instruções (os *getters* do
  `defineLazy` não herdam; a flecha `() => ["root"]` das assinaturas
  estáticas herda).

`DARTFORGE_JSPROD_STRINGS=todas` volta à regra antiga, e
`DARTFORGE_JSPROD_PROPS_POR=a,b` diz por que cada nome fica.

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

`DARTFORGE_JSPROD_ACHATAR=0` desliga.

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
`Ctx::receita_de`. `DARTFORGE_JSPROD_RECEITA_CURTA=0` desliga.

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
`DARTFORGE_JSPROD_RECEITA_CURTA=0` volta à receita `rótulo|Nome`.

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
como antes. `DARTFORGE_JSPROD_FORMAS=0` desliga.

### 9.9 Receitas fechadas do programa avaliadas uma vez

`t$R("receita")` (o `_Universe.eval` de uma receita fechada) aparecia 73 mil
vezes no `new_sali`. No programa (depois da marca do fim do SDK), cada receita
usada duas vezes ou mais vira uma variável `t$T…`, declarada logo depois do
SDK: `_Universe.eval` devolve o objeto canônico do universo, então avaliar
uma vez dá o mesmo objeto, e cada uso fica uma variável em vez de uma
chamada. O SDK fica como está: código dele roda durante a carga, antes da
declaração. Também em `formas.rs`; `DARTFORGE_JSPROD_RECEITAS_FIXAS=0`
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
passo foi medido sozinho, e o corpus `--producao` deu 238/238 depois de cada um.

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

Validação: corpus `--producao` 238/238 (desenvolvimento 238/238), testes de
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

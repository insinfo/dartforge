# JS de produção com o SDK compilado pela nossa trilha

Especificação escrita **antes** do código, depois da medição, como pede o
método do projeto: medir, especificar, implementar e só então validar. O
plano anterior (`docs/JS-PRODUCAO.md`) chegou ao limite que ele mesmo previu
(§1.5 e §6.0 de lá): enquanto o runtime for o `dart_sdk.js` pré-compilado
pelo DDC, o arquivo de produção tem piso de ~1,4 MB, contra ~35 KB do
`dart2js` num `print`. Este documento diz como fechar esse buraco, em que
ordem e com quanto de ganho esperado em cada passo.

Referências lidas para escrevê-lo:

* o compilador do DDC **na versão 3.6.2** (`git show 3.6.2:pkg/dev_compiler`,
  extraído para leitura; a árvore de trabalho de `references/dart-sdk` é mais
  nova e não serve de oráculo). As citações `compiler.dart:N` são dessa
  versão;
* `pkg/compiler` (dart2js): `universe/`, `js_backend/backend_impact.dart`,
  `js_backend/namer.dart`, `js_emitter/`;
* o SDK 3.6.2 (`C:/tools/dartsdk-3.6.2/lib`), seção `dartdevc` do
  `libraries.json`, com os *patches* de `_internal/js_dev_runtime`;
* `references/oxc` (parser, semântica, *mangler*, impressor).

---

## 1. O ponto de partida, medido

### 1.1 Tamanhos (2026-09-30)

`dartforge-jsprod` do `main` (mundo fechado do usuário, poda por membro do
`dart_sdk.js`, compactação de espaço) contra `dart compile js` do SDK 3.6.2.
Bytes.

| programa | jsprod | dart2js -O2 | dart2js -O4 |
| --- | ---: | ---: | ---: |
| `01_print` | 1.369.630 | 37.159 | 34.929 |
| `40_classes_basico` | 1.379.223 | 37.072 | 35.227 |
| `80_async_await_basico` | 1.376.136 | 52.533 | 49.502 |
| `120_convert_json` | 1.551.412 | 60.870 | 58.553 |
| `128_uri` | 1.632.196 | 128.849 | 119.204 |
| `135_convert_json_classes` | 1.718.947 | 105.796 | 101.196 |
| `138_collection_hashmap_ordenado` | 1.396.494 | 70.729 | 65.676 |

Somos **14 a 39× maiores**. Os projetos reais
(`limitless_ui/example`, `new_sali/frontend`) estão em §1.4.

### 1.2 Do que é feito o arquivo de um `print`

`01_print` sem compactação (1.528 KB), por biblioteca de topo:

| o quê | KB |
| --- | ---: |
| `dart:async` | 203 |
| `dart:_rti` | 169 |
| `dart:core` | 141 |
| `dart:_internal` | 102 |
| `dart:_native_typed_data` + `typed_data` (num `print`!) | 94 + ~50 de metadado |
| `dart:_js_helper` | 69 |
| **`dart:_runtime` (o objeto `dart`)** | **61** |
| `dart:_interceptors` | 61 |
| `dart:collection` | 56 |
| assinaturas (`setMethodSignature`, `setGetterSignature`, `setFieldSignature`…) | ~250 |
| código do usuário | 1 |

Três achados decidem o plano:

1. **O runtime escrito à mão do DDC não é o problema**: o objeto `dart` são
   61 KB. O que pesa são as bibliotecas Dart do SDK, vivas porque o alcance
   sobre texto JS é grosseiro — toda *string* de uma unidade viva conta como
   seletor possível (`docs/JS-PRODUCAO.md` §1.4), então `async`, `typed_data`
   e metade do `core` entram num programa que só imprime.
2. **`dart_rti._Universe.eval(dart_rti._theUniverse(), "…", true)` aparece
   2.259 vezes** no `01_print` — ~140 KB só desse prefixo.
3. Um minificador genérico não resolve: o `esbuild --minify` sobre o mesmo
   arquivo dá 1.167 KB (−15%). O que falta é **não emitir**, não encurtar.

### 1.3 Experimento: o nosso emissor diante do SDK

Com um desvio temporário (variável de ambiente, já removido) o emissor de
`crates/emit_js` emitiu as bibliotecas do SDK com os corpos inferidos pela
`crates/types`: `core` (382 KB), `async` (463 KB), `_interceptors`,
`_js_helper`, `_internal`, `_rti`, `math`, `convert`, `_js_primitives` saíram
sem erro; `collection` estoura a pilha. A forma é a do DDC (as mesmas
classes, construtores, assinaturas, receitas rti). As diferenças são as que o
DDC trata como caso especial ao compilar o SDK — chamadas a `JS()` saem como
chamadas comuns a `_foreign_helper.JS(...)`, membros de classes nativas saem
com nome Dart em vez do símbolo `dartx`, falta o `registerExtension`.
Comparação com o `dart_sdk.js` do mesmo `JSBool`:

```js
// nosso                                   // DDC
toString() {                               [$toString]() {
  return _foreign_helper.JS(<rti>,           return String(this);
    "!", "String(#)", this);               }
}
```

O catálogo completo dessas diferenças, com as linhas do `compiler.dart`
3.6.2, está em §5. São finitas e todas de um destes tipos: intrínsecos de
compilação (`JS`, `JS_GET_FLAG`…), classes nativas, a biblioteca do runtime,
nomes de biblioteca. Nenhuma pede um emissor novo.

### 1.4 Projetos reais

`limitless_ui/example` pelo perfil de produção de antes: 45.113 KB
(`docs/JS-PRODUCAO.md` §6.0), contra 4.511.811 B do build oficial
(`build_web_compilers --release`, dart2js). Aqui o SDK é 3% do arquivo; o
resto é código do usuário e dos pacotes no contrato do DDC, sem minificação e
com metadado completo. A medição de `new_sali/frontend` e a de depois estão
em §10.

---

## 2. O que o dart2js faz e nós não

1. **Compila o SDK junto com o programa.** `_js_helper`, `_interceptors` e
   `_rti` são bibliotecas Dart comuns que passam pelo mesmo mundo fechado; o
   que as mantém vivas são os impactos declarados em
   `js_backend/backend_impact.dart` (o que o próprio backend escreve), não
   uma regra de "sempre incluir".
2. **Mundo fechado sobre elementos**, com seletor por nome, espécie e tipo do
   receptor (`universe/`).
3. **Escreve à mão só o preâmbulo**: ~350 linhas de
   `js_emitter/startup_emitter/fragment_emitter.dart`. Todo o resto do
   runtime é Dart com `JS()`.
4. **Emite pouco metadado**: assinatura de função só onde a reflexão de tipo
   em execução precisa.
5. **Minifica nomes**: locais pelo escopo e membros por um *namer* global
   com lista de fugas (`docs/JS-PRODUCAO.md` §3).

Nós temos 2 para o código do usuário (`crates/mundo`). Este plano traz 1, 3 e
4 para o perfil de produção e a parte segura de 5.

---

## 3. Arquitetura

### 3.1 Três camadas

```
┌──────────────────────────────────────────────────────────────┐
│ bootstrap escrito à mão (~60 linhas)                          │
│   dart, dart.library, dart.privateName, dartx, namespaces     │
├──────────────────────────────────────────────────────────────┤
│ SDK em Dart, compilado pelo NOSSO emissor (modo SDK)          │
│   dart:_runtime, _rti, core, _interceptors, _js_helper,       │
│   _internal, async, collection, convert, math, typed_data,    │
│   _native_typed_data, js_util, js_interop, html, …            │
│   — só o que o mundo fechado alcança                          │
├──────────────────────────────────────────────────────────────┤
│ programa (usuário + pacotes), o emissor de sempre             │
└──────────────────────────────────────────────────────────────┘
```

**Nenhum byte do `dart_sdk.js` entra no arquivo.** Ele continua existindo
para o perfil de desenvolvimento e como oráculo; o perfil de produção ganha
`--sdk-ddc` para o caminho antigo (comparação e recuo).

### 3.2 Qual runtime escrever à mão: só o bootstrap

A pergunta "qual runtime JS mínimo escrever à mão" tem resposta medida: **o
bootstrap e nada mais**. O runtime do DDC (`dart:_runtime`, 3.800 linhas em
`ddc_runtime/*.dart`) já é JavaScript escrito à mão — dentro de `JS('',
'''…''')` — com Dart em volta. Reescrevê-lo em JS daria o mesmo texto que a
compilação dele produz, e perderia a granularidade: compilado pela nossa
trilha, cada função do runtime é um elemento do mundo fechado e só entra se
alguém a chama. É exatamente a escolha do dart2js (o runtime é Dart com
`JS()`, o preâmbulo é à mão).

O bootstrap é o que o próprio DDC gera sem fonte Dart (`compiler.dart:
7934-7989`): `const _library = Object.create(null); const dart =
Object.create(_library); dart.library = _library;`, um `var X =
Object.create(dart.library)` por biblioteca, o `dartx`, o
`dart.privateName` e a declaração dos símbolos `dartx.X = Symbol("dartx.X")`.

### 3.3 O contrato continua sendo o do DDC

O emissor é o mesmo do perfil de desenvolvimento, e os *templates* JS do
runtime (`JS('', '#[dartx.toString]()', …)`, `registerExtension`, `$ti`,
`dart.fn`) supõem o contrato do DDC. Mudar de contrato (para o modelo de
objetos do dart2js) seria um backend novo; não é este plano. Consequência
honesta, registrada em §11: o contrato do DDC carrega custos que o do dart2js
não tem (receita rti por classe, `dart.fn` em cada *closure*), e o alvo
realista é ficar **de 2 a 4× do dart2js**, não
empatar.

---

## 4. Mundo fechado do programa inteiro

`crates/mundo` deixa de tratar o SDK como fronteira. Com
`Opcoes::incluir_sdk` (`calcular_com`), as classes, funções e variáveis das
bibliotecas `dart:` entram no ponto fixo com as mesmas regras das do usuário
(§1.7 de `docs/JS-PRODUCAO.md`): três níveis de classe, seletor por nome com
espécie e cone do receptor, pendências por nome. Sem a opção o crate é byte
a byte o de antes (o perfil `--sdk-ddc` e o modo de desenvolvimento não
mudam).

### 4.1 Corpos do SDK

A inferência de corpos passa a cobrir as bibliotecas do SDK
(`infer_bodies_das_bibliotecas` com todas as bibliotecas — o mesmo que o
backend nativo já faz). Medido: o SDK inteiro são ~120 mil expressões, 155 ms.

### 4.2 O que o programa Dart não enxerga: impactos do backend

O emissor escreve chamadas que não estão na AST: `dart.fn` em cada
*closure*, `_interceptors.JSArray.of` num literal de lista,
`_js_helper.LinkedMap` num de mapa, `dart.str` na interpolação, `dart.dsend`
no despacho dinâmico, `async._asyncStartSync`… O dart2js resolve isso com a
lista de `backend_impact.dart`. Nós resolvemos **pelo texto**, com o
mecanismo que já existe para o código do usuário (`verificar.rs`):

1. o mundo é calculado;
2. o programa e o SDK são emitidos com o mundo como filtro;
3. cada referência do texto emitido (`dart.X`, `core.Y`, `lib['Ext|m']`,
   receitas rti `"core|Z"`, nomes em `dsend`/`dload`/`dput`/`bind`) é
   traduzida para o elemento que a define; a que aponta para algo morto
   volta como raiz;
4. repete até o ponto fixo.

Para não pagar dez rodadas, as raízes iniciais já trazem os impactos fixos
(a semente): `core.Object`, os interceptadores registrados por tipo embutido
do JS (`JSString`, `JSNumber`, `JSBool`, `JSArray`, `JSNull`,
`JavaScriptObject`, `LegacyJavaScriptObject`, `UnknownJavaScriptObject`,
`JavaScriptFunction`), o que o bootstrap chama ao carregar e as funções do
runtime citadas pelo texto do emissor que não dependem do programa. A
semente é otimização; a correção vem do ponto fixo do texto, que não depende
de lista nenhuma.

### 4.3 Classes nativas

Um objeto do DOM ou um *array* JS não nasce de `new` no programa: nasce no
navegador e vira instância Dart pelo `registerExtension` do tag. Regra (a
mesma ideia de `NativeBehavior` no dart2js, na forma conservadora): **classe
nativa viva como tipo é instanciada** — os membros dela vivem pela regra de
seletor como os de qualquer classe instanciada. Uma classe nativa que nenhum
código cita não precisa existir: o objeto herda, pela cadeia de protótipos
do JS, os membros instalados na superclasse registrada (`HTMLDivElement` →
`HTMLElement`, que tem os de `HtmlElement`).

### 4.4 O runtime chamando por nome

Três fontes de nome que o Dart não vê, todas extraídas do texto **do
runtime que nós mesmos emitimos** (não mais do `dart_sdk.js` inteiro):

* *strings* de `dsend`/`dgsend`/`dload`/`dput`/`bind` (regra (iii) de
  `docs/JS-PRODUCAO.md` §1.7);
* propriedades citadas nos *templates* `JS()` (`#.moveNext()`,
  `#.current`, `#[dartx.toString]`) — o verificador as cura se faltarem;
* membros de `Object` e `call`, universais como antes.

A regra (i) do contrato antigo — "todo membro declarado num supertipo do SDK
de uma classe instanciada está vivo" — **deixa de existir** com o SDK
incluído: quem chama `compareTo` é código do SDK que agora o mundo enxerga,
com o cone do receptor.

---

## 5. Modo SDK do emissor

O emissor é o de `crates/emit_js`. O que ele precisa para compilar o SDK
fica num arquivo novo, `crates/emit_js/src/sdk_proprio.rs`, chamado por
ganchos pequenos nos pontos listados abaixo. **Sem o modo SDK nenhum gancho
dispara** e a emissão é byte a byte a de antes (teste
`emit_js_producao/tests/identidade.rs` e o corpus inteiro contra o binário
anterior).

Cada item tem a referência do DDC 3.6.2 e é marcado **[S]** (semântico:
sem ele o programa erra) ou **[T]** (só tamanho ou forma; não se faz).

### 5.1 Intrínsecos (`dart:_foreign_helper` e afins)

| chamada | saída | ref. |
| --- | --- | --- |
| `JS(tipo, 'template', a…)` [S] | o *template* com `#` trocado pelos argumentos (parênteses pela precedência; o *template* é JS de verdade, então os argumentos entram entre parênteses sempre que não forem primários). Em posição de instrução, sai como instrução; `throw …` é instrução. *Template* com interpolação Dart (`'''…$x…'''`) vira `#` na ordem. Argumentos com `_isInForeignJS`: literal de tipo sai como rti cru (sem `createRuntimeType`) e *tearoff* estático sai cru (sem `dart.fn`) | `compiler.dart:6421-6479`, `:7017`, `:7366` |
| `JS_GET_FLAG('X')` [S] | `DEV_COMPILER` → `true`; `SOUND_NULL_SAFETY` → `true`; `MINIFIED`, `LEGACY`, `EXTRA_NULL_SAFETY_CHECKS`, `PRINT_LEGACY_STARS` → `false`; `VARIANCE` → `true`. Condição literal poda o `if`/`?:` | `:6147-6177`, `:4726-4744` |
| `JS_GET_NAME(JsGetName.X)` [S] | `OPERATOR_IS_PREFIX` → `"$is_"`, `SIGNATURE_NAME` → `dart._functionRti`, `RTI_NAME` → `"$ti"`, `FUTURE_CLASS_TYPE_NAME` → `"async\|Future"`, `LIST_CLASS_TYPE_NAME` → `"core\|List"`, `RTI_FIELD_AS`/`_IS` → símbolos privados `_as`/`_is` de `dart:_rti` | `:6495-6546` |
| `JS_EMBEDDED_GLOBAL('', N)` [S] | `arrayRti` → `$arrayRti` (símbolo `dartx`); outro → `dart.<N>` | `:6481-6491` |
| `JS_CLASS_REF(T)` [S] | a classe (`core.Object`); `Null` → `core.Null` | `:6128-6141` |
| `TYPE_REF<T>()` / `LEGACY_TYPE_REF<T>()` [S] | a rti de `T` (`T*` no segundo) | `:6108-6114` |
| `RAW_DART_FUNCTION_REF(f)` [S] | a função, crua | `:6142-6146` |
| `JS_STRING_CONCAT(a, b)` [S] | `a + b` | `:6180-6184` |
| `JS_BUILTIN(d, JsBuiltin.X, …)` [S] | `dartClosureConstructor` → `Function`, `dartObjectConstructor` → `core.Object` | `:6523-6534` |
| `JS_RAW_EXCEPTION()` [S] | a variável do `catch` JS | `:6191-6194` |
| `JS_RTI_PARAMETER()` [S] | `_ti` | `:6195-6197` |
| `DART_RUNTIME_LIBRARY()` [S] | `dart` | `:6104-6106` |
| `getInterceptor(o)` [S] | `dart.getInterceptorForRti(o)` | `:6119-6122` |
| `extensionSymbol('x')` (runtime) [S] | `$x` (o símbolo `dartx.x`) | `:6200-6218` |
| `_jsInstanceOf(x, T)` (runtime) [S] | `x instanceof T` | idem |
| `jsObjectGetPrototypeOf/SetPrototypeOf` (`_js_helper`) [S] | `Object.getPrototypeOf(o)`/`setPrototypeOf` | `:6221-6232` |
| `staticInteropGlobalContext` [S] | `dart.global` | `:5244-5249` |
| `unsafeCast<T>(x)` (`_internal`) [S] | `x` | `:6048-6096` |
| `extractTypeArguments<T>(i, f)` [S] | `dart.dgcall(f, [rti…], [])` | idem |
| `_getPropertyTrustType`, `_setPropertyUnchecked`, `_callMethodUnchecked*`, `_callConstructorUnchecked*` (`js_util`) [S] | `o[n]`, `o[n] = v`, `o[n](a…)`, `new c(a…)` — são `external` sem corpo | `:6251-6302` |
| `spread(x)` / `@rest` [S] | `...x` / `...p` | `:6400-6403`, `:3722-3726` |
| inlining de `dart:_rti` [T] | — | `:6001-6046` |
| atalhos de fábrica `Map()`/`List()` [T] | — (os *patches* têm corpo) | `:6668-6709` |

Os `external` de `_foreign_helper` sem corpo (`JS_OPERATOR_IS_PREFIX`…)
nunca são emitidos (`:3096-3099`).

### 5.2 Classes nativas e símbolos `dartx` (`native_types.dart`)

* **Conjunto nativo** [S]: `int`, `double`, `bool`, `String` e toda classe
  com `@Native`/`@JsPeerInterface` em `_interceptors`, `_native_typed_data`,
  `html`, `indexed_db`, `svg`, `web_audio`, `web_gl`. **Extensíveis**:
  `Object`, `Function`, `Comparable`, `Map`, `ListBase`, `MapBase`,
  `Rectangle` e todo supertipo de nativa. O nosso `Ctx::ext_set` já é esse
  conjunto (é como o código do usuário chama `list[$add]`).
* **Declaração** de membro de instância em classe nativa usa o símbolo
  `dartx`: `[$toString]()`, `get [$hashCode]()` [S] (`compiler.dart:2771`).
  Operadores: `$plus`, `$bitAnd`… — a tabela do nosso `dartx_var` já é a de
  `friendlyNameForDartOperator`.
* **Epílogo** da classe nativa [S]: nada de `defineExtensionMethods`; para
  `JSBool`/`JSNumber`/`JSString`, `dart.definePrimitiveHashCode(C.prototype)`;
  para cada *peer* do `@Native('A,B')` (sem os que começam com `!`),
  `dart.registerExtension("A", C)`. Para `Object`,
  `dart._installIdentityEquals()` (`:1108-1117`).
* **Corpos especiais** [S]: `Object` com
  `constructor() { throw Error("use `new " + … + ".new(...)` to create a Dart object"); }`;
  `JSArray` com `constructor() { return []; }` (`:2163-2183`).
* **Membros `external` de classe nativa** [S]: *getter* → `return
  this.<js>;`, *setter* → `this.<js> = v;`, método → `return
  this.<js>.apply(this, args);` com `@JSName` dando o nome JS; em biblioteca
  web, o retorno não anulável vai por `dart.checkNativeNonNull` (`:2322-2343`,
  `:6469-6474`). Campos de classe nativa viram par *getter*/*setter* sobre
  `this.<js>`. Estáticos `external` → `dart.global.<Peer>.<js>(…)`
  (`:6371-6385`); de topo com `native` → `dart.global.self.<js>`.
* **Declaração dos símbolos** [S]: o bootstrap declara
  `dartx.X = Symbol("dartx.X")` para todo símbolo usado em qualquer módulo do
  arquivo (o nosso e o do SDK). **Cada símbolo é criado uma vez só** — dois
  `Symbol()` com o mesmo nome são chaves diferentes.

### 5.3 A biblioteca `dart:_runtime`

* O objeto da biblioteca **é** `dart` [S] (não `Object.create(dart.library)`).
* Funções de topo: `dart.<nome> = function <nome>(…)`, com `@JSExportName`
  dando o nome da propriedade (`dart.throw` ↔ `throw_`) [S].
* Nomes privados de topo são propriedades comuns (`dart._mixin`), não
  símbolos [S].
* `@ReifyFunctionTypes(false)` na biblioteca: nenhuma *closure* do runtime
  ganha `dart.fn` [S] (o runtime usa funções como objetos JS crus).
* Campos de topo: ansiosos (`dart.x = <init>;` no lugar) quando o
  inicializador é nulo, literal, `JS()` ou construção de classe do próprio
  runtime; o resto num `dart.defineLazy(dart, {…})` [S] (`:2635-2681`).
* Ordem [S]: `dart.typeUniverse = {eC: new Map(), tR: {}, eT: {}, tPV: {},
  sEA: []};` primeiro; depois as funções; depois os campos ansiosos; as
  classes do runtime depois de `core.Object` (elas o estendem).

### 5.4 Nomes de biblioteca

`dart:_runtime` → `dart`, `dart:_rti` → `dart_rti`, `dart:html` → `html`,
`dart:svg` → `svg` (o `$` do DDC só existe porque o `TemporaryNamer` dele
colide com locais; o nosso `js::ident` já renomeia locais com esses nomes),
as outras pelo caminho (`_interceptors`, `core`). São os mesmos nomes que o
código do usuário já usa para importar do `dart_sdk.js`.

### 5.5 O resto do catálogo

* `@nullCheck` em parâmetro → `if (p == null) dart.argumentError(p);` [S]
  (`:3855`): é o que faz `1 + null` dinâmico lançar `ArgumentError` em vez de
  dar `NaN`.
* Super-chamada de construtor: a nossa (`Pai.new.call(this)`) é equivalente
  à do DDC (`dart.global.Object.getPrototypeOf(C).new.call(this)`) [T].
* Fábricas privadas do SDK que só lançam `UnsupportedError` [T].
* `_CovarianceTransformer` (`target.dart:320-521`): tira conferências de
  covariância de membros privados nunca chamados por fora de `this` [T]
  (conferir a mais nunca muda o resultado de um programa correto).
* Ordem de emissão das classes por biblioteca, tabelas `S`/`I`, `findType`
  ×3, `trackLibraries` do SDK [T].

### 5.6 Montagem do módulo do SDK

As bibliotecas do SDK formam um único componente fortemente conexo de
imports; saem num módulo só, na ordem: bootstrap → funções e campos
ansiosos de `dart:_runtime` → classes (todas as bibliotecas, superclasse
antes) → resto de cada biblioteca (funções, `copyProperties`, `defineLazy`)
→ regras rti → os `findType` do DDC. Os módulos do programa vêm depois, como
hoje (IIFE por módulo, ordem topológica).

---

## 6. Emissão enxuta de produção

Ganchos no emissor guardados por `Ctx::producao` (desligado no
desenvolvimento). Cada regra cita quem consome o metadado no runtime.

1. **Assinaturas por demanda.** `setMethodSignature`, `setGetterSignature`,
   `setSetterSignature` e `setFieldSignature` são lidas por `getMethodType`,
   `getSetterType` e `getFieldType` (`ddc_runtime/classes.dart:200-262`), que
   só o despacho dinâmico (`dsend`, `dload`, `dput`, `dcall` de membro) e o
   *tearoff* (`bind`) chamam — e o depurador, que a produção não tem. Uma
   entrada de assinatura vive se o nome do membro está entre os **seletores
   dinâmicos vivos** do arquivo (os nomes de `dsend`/`dgsend`/`dload`/`dput`/
   `bind` no texto vivo, inclusive o do runtime). O resto sai. A mesma regra
   para estáticos (`setStatic*Signature`), que só `dload`/`dput` em tipo leem.
2. **`dart.trackLibraries`** e **`dart._checkModuleNullSafetyMode`** saem:
   servem ao depurador e à checagem de modo entre módulos compilados à parte,
   que num arquivo só não existem.
3. **Alias das receitas rti.** `dart_rti._Universe.eval(dart_rti._theUniverse(),
   "R", true)` vira `$T("R")`, com `$T` declarado uma vez no bootstrap. É a
   mesma chamada; o ganho é o texto (§1.2).

Fica (consumido em execução): `addRtiResources` (testes de tipo),
`setLibraryUri` (mensagem de `TypeError` quando dois tipos têm o mesmo nome,
`errors.dart:113`), `defineExtensionMethods`/`Accessors`, regras rti.

---

## 7. Minificação

### 7.1 Espaço e comentários

Já existe (`minificar.rs`, −12%). Fica como está e corre por último.

### 7.2 Identificadores

Locais, parâmetros, funções e classes **por escopo**, sobre o arquivo
inteiro, com o `oxc` (decisão de `docs/JS-PRODUCAO.md` §3.3): `oxc_parser` →
`oxc_semantic` → `oxc_mangler` → `oxc_codegen`. O arquivo é embrulhado numa
função (`(function(){…})();`) para que os nomes de topo — `dart`, `core`,
`L$pacote__lib__arquivo`, `$T` — também sejam locais e possam encurtar; nada
de fora do arquivo os cita. Globais livres (`self`, `window`, `console`,
`dartPrint`, os construtores do DOM) não são renomeados por construção (não
têm declaração no arquivo). Escopos com `eval` direto ficam intactos (o
`oxc` já recusa).

**Propriedades não são renomeadas** neste plano. O nome de membro Dart é
observável no contrato do DDC — `dsend(o, "foo")` procura a propriedade
pelo nome de origem, `NoSuchMethodError` o imprime, `@JSName`/interop/DOM o
exigem literal, os *templates* `JS()` citam propriedades — e a única forma
segura seria a lista de fugas inteira de `docs/JS-PRODUCAO.md` §3.2 mais uma
tabela nome→curto para todo seletor dinâmico. É o próximo passo de tamanho
depois deste plano, não parte dele.

---

## 8. Despacho direto por alvo único

`docs/JS-PRODUCAO.md` §2 dá as guardas de `locateSingleMember`. Com o SDK no
mundo, o conjunto de alvos de um seletor é exato. A troca de
`dart.dsend(o, "foo", a)` por `o.foo(a)` exige, todas juntas:

1. o seletor não é `call` de *closure*;
2. exatamente **um** membro vivo com esse nome em todo o programa, e ele não
   é `noSuchMethod` de nenhuma classe instanciada (uma classe com
   `noSuchMethod` próprio é alvo de qualquer seletor);
3. a aridade da chamada bate exatamente com a do alvo (sem opcionais
   faltando, sem nomeados);
4. cada argumento é estaticamente atribuível ao parâmetro do alvo (senão a
   conferência de `_checkAndCall` que lança `TypeError` se perderia);
5. o receptor é provado não nulo e de uma classe que tem o alvo — o que, com
   receptor `dynamic`, não temos como provar sem análise de fluxo.

A guarda 5 é a que decide, e é por isso que `docs/JS-PRODUCAO.md` §2 já
mediu zero sítios no corpus: quando o tipo estático do receptor é conhecido,
o emissor já chama direto (`o.foo(a)`); o `dsend` só sobra onde o receptor é
`dynamic`, e aí a 5 não se prova sem análise de fluxo. O que este plano faz:
as guardas 1–4 calculadas sobre o mundo, num **relatório** de quantos sítios
`dsend` passariam se a 5 fosse provada — o número que diz se uma análise de
fluxo para receptores dinâmicos vale o custo. Nenhuma troca é feita sem a 5.
Ganho esperado em tamanho: zero nos programas medidos.

## 9. Deduplicação

`docs/JS-PRODUCAO.md` §3.5: na trilha tipada, nunca no texto do
`dart_sdk.js`; Safe ICF (identidade observável preservada). No contrato do
DDC, um método de instância não tem identidade observável — o *tearoff* é
um `dart.bind` novo a cada vez, e a igualdade de *tearoffs* compara receptor
e nome — então dois métodos de classes diferentes **do mesmo módulo** com
corpo idêntico, sem `super` (o `super` liga ao *home object*) e sem
referência ao nome da classe, podem dividir o mesmo objeto função:
`D.prototype.m = C.prototype.m`. A comparação é sobre o corpo emitido pelo
nosso emissor a partir da AST tipada, normalizado só nos nomes de
parâmetros, dentro de um módulo (os nomes livres resolvem para o mesmo
armazenamento dentro da mesma IIFE — a regra 2 da pesquisa). Ganho esperado:
pequeno (<3%); o passo mede antes e só fica ligado se ganhar.

## Fora deste plano, com o porquê

* **Carregamento diferido** (`deferred` → `import()`): nenhum programa do
  corpus, do `limitless_ui` ou do `new_sali/frontend` usa `deferred`
  (conferido por busca). Sem consumidor não há como validar.
* **Renomear propriedades** (§7.2).
* **Trocar o contrato do DDC pelo do dart2js** (§3.3).

---

## 10. Ordem, ganho esperado e critério de saída

| # | passo | ganho esperado no `01_print` | critério de saída |
| --- | --- | --- | --- |
| 1 | mundo fechado com o SDK (§4) + modo SDK do emissor (§5) + montagem (§5.6): o arquivo deixa de ter `dart_sdk.js` | 1.370 KB → **~250-400 KB** (o SDK passa a ser podado por elemento, e `typed_data`/`async` saem de um `print`) | corpus/js inteiro igual à VM; `--sdk-ddc` byte a byte o de antes |
| 2 | emissão enxuta (§6) | −25 a −35% do que sobrar (assinaturas ~17% e o prefixo rti ~9% hoje) | idem |
| 3 | minificação de identificadores (§7.2) | −30 a −40% do que sobrar | idem + `node --check` |
| 4 | despacho direto (§8) | ~0 | idem; relatório de sítios |
| 5 | deduplicação (§9) | <3% | idem; ligada só se ganhar |

Alvo do conjunto: `01_print` **na casa de 80-150 KB**
(de 1.370 / 176 KB), ou seja, 2,5 a 4× o dart2js em vez de 40×; nos
projetos reais, o ganho dos passos 2 e 3 vale sobre o código do usuário, que
lá é 97% do arquivo.

## 11. Verificação

1. `cargo run --release -p dartforge-diferencial -- --producao`: o corpus
   inteiro com o mesmo `stdout` da VM, byte a byte.
2. Os testes do crate (`identidade.rs` garante que sem o modo SDK nada muda).
3. `limitless_ui/example` e `new_sali/frontend` no navegador, pelo
   `scripts/fluxo.mjs`/`sondar-limitless-ui.mjs` do repositório.
4. Tamanhos antes/depois contra o dart2js (bruto), na mesma tabela de
   §1.1.
5. Determinismo: duas montagens iguais dão o mesmo arquivo (teste de
   unidade).

---

## 12. Resultado (2026-10-01)

O SDK próprio é o **padrão** do `dartforge-jsprod`. `--sdk-ddc` (ou
`DARTFORGE_JSPROD_SDK=ddc`) volta ao `dart_sdk.js` podado, e
`--verificar-stub` também usa o caminho antigo: o *stub* não existe no SDK
próprio.

### 12.1 Verificação

| item | resultado |
| --- | --- |
| `dartforge-diferencial --producao`, SDK próprio | **238/238** iguais à VM |
| o mesmo com `--sdk-ddc` | 238/238 |
| testes de `emit_js_producao`, `mundo`, `emit_js`, `elements`, `types` | verdes |
| `limitless_ui/example` em produção, suíte `ui_test/e2e` (Puppeteer) | **26/26** |
| `new_sali/frontend` em produção, `scripts/fluxo.mjs` | 11 passos, 0 com erro, igual ao `dart2js -O4` |

### 12.2 Tamanhos

Bytes. A coluna "antes" é a §1.1.

| programa | antes | agora | dart2js -O4 |
| --- | ---: | ---: | ---: |
| `01_print` | 1.369.630 | **257.738** | 34.929 |
| `120_convert_json` | 1.551.412 | **408.091** | 58.553 |
| `135_convert_json_classes` | 1.718.947 | **522.234** | 101.196 |
| `limitless_ui/example` | ~45,1 MB | **14.043.321** | 4.511.811 ¹ |
| `new_sali/frontend` | — | **24.895.868** | 7.616.039 ² |

¹ O build oficial (`build_web_compilers --release`).
² `dart compile js -O4` sobre uma cópia com os `.template.dart` gerados
ao lado das fontes (sem `build_runner`).

Em relação ao antes, o `print` ficou 5,3× menor. O
`limitless_ui` ficou 3,2× menor. A meta do conjunto (§10: 80-150 KB bruto
no `01_print`) **não foi atingida**. Composição do que sobra no `01_print`,
antes da troca de nomes do `oxc`:

* `dart:_rti`: 92 KB, o motor de tipos inteiro;
* `core`: 34 KB;
* `_js_helper`, `_internal` e `_interceptors`: 25 KB cada;
* o objeto `dart`: 16 KB;
* *strings*: 100 KB dos 257 KB, em receitas rti, nomes de `dartx` e
  `setLibraryUri`.

O que falta para chegar perto do `dart2js` é o que a §7.2 deixou fora:
renomear propriedades e enxugar o motor rti.

### 12.3 Correções que a validação pediu

* **Escopo dos patches do SDK** (`crates/elements/src/outline.rs`,
  `escopos_de_patch`): os imports de um arquivo de patch valem nele e
  escondem os da biblioteca, como no CFE. O
  `js_interop_unsafe_patch.dart` importa `dart:_foreign_helper show JS` e
  `dart:js_interop hide JS`. Antes, `JS(...)` resolvia para a anotação
  `@JS` (`238_interop_is_a`).
* O único `assert` do `dart:_runtime` (`assertInterop`) fica, como no
  `dart_sdk.js` (`209_js_interop`).
* **Nome da expressão de classe**: quando o corpo cita o próprio nome como
  identificador livre (`new ResizeObserver(#)` no `_create_1`), a classe sai
  como `class ResizeObserver$`. É o que faz o DDC, e sem isso o *template*
  construía a própria classe Dart (14 testes do `limitless_ui`).

### 12.4 Pendências

1. **Tempo do mundo fechado nos projetos reais**: 7,6 min no
   `limitless_ui` e 11,7 min no `new_sali`. O `dart compile js -O4` do
   `new_sali` leva 55 s.
2. **Tamanho**: motor rti enxuto e minificação de nomes de propriedade.
3. Relatório do despacho direto (§8) e medição da deduplicação (§9).

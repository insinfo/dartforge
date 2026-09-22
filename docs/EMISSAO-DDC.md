# Emissão JavaScript no contrato do DDC

Decisão de 2026-09-22, sob a regra "funcionar primeiro": o DartForge emite
**módulos ES6 no mesmo contrato que o DDC emite**, e liga contra o
`dart:*` oficial compilado pelo DDC (`runtime/ddc/dart_sdk.js`, gerado por
`scripts/gerar-dart-sdk.ps1` a partir do `ddc_platform.dill` do SDK 3.6.2).
Verificado: o `hello.js` que o `dartdevc` produz roda no Node 24 contra
esse arquivo e imprime o mesmo que `dart run`.

Por que isso e não um runtime próprio: o runtime é o Dart oficial inteiro
(`dart:core`, `async`, `collection`, `convert`, `html`…), com semântica
exata, hoje. O passo 4 do PLANO (compilar o SDK pela nossa própria fonte)
continua e entra **com o mesmo contrato** — o `dart_sdk.js` é trocado, o
que se emite não muda. Modo de produção (programa inteiro, tree shaking) é
etapa posterior sobre a mesma emissão.

## O oráculo é o `dartdevc`

Para qualquer construto, a forma certa de emitir é a que o `dartdevc`
emite. Ele está no SDK e leva 0,3 s:

```
dart C:/tools/dartsdk-3.6.2/bin/snapshots/dartdevc.dart.snapshot --modules=es6 -o saida.js entrada.dart
```

Regra de trabalho: **antes de emitir um construto, compile um exemplo
mínimo com o `dartdevc` e copie a forma**. Não invente convenção. O que
importa não é o texto ser idêntico, é o runtime (`dart.*`, `dartx.*`,
`dart_rti`) receber exatamente o que espera.

## Pipeline

```
frontend (Ast) → elements (Program) → types (OutlineTypes, BodyTypes) → emit_js → *.js
```

`crates/emit_js` (`dartforge-emit-js`):

```rust
pub struct Emitido { pub modulos: Vec<(String /*nome*/, String /*js*/)>, pub entrada: String /*main.mjs*/ }
pub fn emitir_programa(program: &Program, interner: &Interner, table: &TypeTable,
    core: &CoreTypes, outline: &OutlineTypes, bodies: &BodyTypes) -> Result<Emitido, Vec<Diagnostic>>;
```

Um módulo ES6 por biblioteca Dart (a biblioteca e seus `part`s), nome do
módulo derivado da URI (`package:x/y.dart` → `packages/x/y.js`,
`file:///…/main.dart` → `main.js`), `import { core, dart, dartx, … } from
'./dart_sdk.js'`, e `main.mjs` que importa o módulo da entrada e chama
`main()` (com `dart.async`/`await` se `main` for `async`). Bibliotecas do
SDK **não são emitidas**: são as do `dart_sdk.js`.

CLI: `dartforge compile-js <entrada.dart> -o <dir> [--sdk <lib>]
[--packages <package_config.json>]` escreve os módulos, copia
`runtime/ddc/dart_sdk.js` para `<dir>` e devolve 0; `node <dir>/main.mjs`
executa.

## O que o contrato exige (lista viva, cada item verificado com o oráculo)

* Classes: `lib.C = class C extends core.Object { … }`, construtores como
  `(lib.C.new = function(...) {…}).prototype = lib.C.prototype`, nomeados
  `lib.C.nome`, campos por símbolo privado `dart.privateName(lib, "C.x")`
  com getter/setter, `dart.addRtiResources`, `dart.setMethodSignature`,
  `dart.setFieldSignature`, `dart.setLibraryUri`; estáticos, getters,
  setters, operadores (`['+'](o)`, `_equals`), `super`, mixins
  (`dart.mixin`/`dart.applyMixin`), classes genéricas (`dart.generic`),
  enums, classes abstratas/interfaces, `noSuchMethod`.
* Chamadas: receptor de tipo nativo JS (`int`, `double`, `num`, `bool`,
  `String`, `List`/`JSArray`, `Null`, funções) usa **`dartx`**
  (`s[dartx.length]`, `l[$map](...)`); receptor `dynamic` usa
  `dart.dsend`/`dart.dload`/`dart.dput`/`dart.dindex`; receptor de classe
  Dart chama direto. Operadores em `int`: `+` vira `+` quando os dois lados
  são `int` (o DDC checa e emite `dart.notNull` onde precisa), `~/` vira
  `(a / b).truncate()` via `dartx`/`dart.ntruncdiv`… copiar do oráculo.
* Closures: `dart.fn(f, receita)`; tearoffs; funções genéricas com
  `dart.gFn`.
* Tipos em execução: receitas de `_Universe.eval` (`"core|int"`,
  `"core|List<core|int>"`, `"core|int(core|String)"`), `dart.is`/`dart.as`,
  `dart.nullCheck`, literais tipados (`_interceptors.JSArray.of(rti, [...])`,
  `new (core.Map$(...)).new()` → copiar).
* Strings: `"a" + dart.strSafe(x)` / `dart.str(x)` para interpolação;
  literais com escapes JS; `DartStr` com surrogate solto vira `\uD800`.
* `async`/`await`: `dart.async(...)`, `await` → `yield` dentro do gerador
  que o DDC emite; `async*`/`sync*` idem, copiar do oráculo.
* Controle: `try/catch` com `dart.getThrown`/`dart.stackTrace`, `rethrow`,
  `switch` clássico e com padrões, rótulos, `assert` (ativo:
  `dart.assert(...)`), `late` (`dart.throwLateInitializationError`),
  cascatas, null-aware, records (`dart.record`?), extension methods
  (funções estáticas do módulo), `const` canonicalizado (`dart.const`,
  tabela `CT`).

## Harness diferencial e corpus

`crates/diferencial` (`cargo run -p dartforge-diferencial`) roda cada programa de
`corpus/js/` em três executores e compara stdout e código de saída byte a byte:

* `dart run --enable-asserts` (semântica; asserts ligados porque o DDC os liga);
* `dartdevc --modules=es6` + `node main.mjs` (contrato; o `main.mjs` importa o módulo,
  chama `main()` e sai com 255 num erro não capturado, como a VM);
* `dartforge compile-js arquivo -o dir` + `node dir/main.mjs`.

Relatório: `ok`/`FALHA` por programa com a primeira linha divergente e os três stdouts
lado a lado; no fim, N/total e as falhas agrupadas pela primeira linha do stderr do
DartForge — é por esse agrupamento que se prioriza o emissor. Oráculos em cache por
hash do conteúdo em `target/diferencial/cache/`; paralelo por núcleo.

* `dartforge-diferencial verificar` — só os oráculos (o corpus tem de rodar na VM e bater
  com o DDC; `// diverge-ddc: motivo` na linha 2 declara a exceção e faz do DDC a
  referência do DartForge para aquele programa).
* `dartforge-diferencial contrato` — regenera `docs/CONTRATO-DDC.md`: o Dart e o JS do
  `dartdevc` de cada programa, sem o preâmbulo repetido. Consultar antes de emitir um
  construto.
* `cargo test -p dartforge-diferencial --test corpus -- --ignored --nocapture` — os dois
  testes do corpus (VM válida em 100%; DDC = VM salvo divergências declaradas).

Corpus: um construto por arquivo, `NN_tema.dart` (ou `NN_tema/main.dart` para várias
bibliotecas), numerado por tema na ordem da lista acima; `140+` são os fixtures antigos
convertidos. Regras para novos programas: saída determinística, sem `dart:io`, sem
imprimir doubles inteiros (`1.0` na VM é `1` na web), `int` dentro de 2^53, bits só com
operandos não negativos, sem `hashCode`/`runtimeType` de tipos do SDK, sem mensagens de
erros do core.

## Critério de aceite

`crates/emit_js/tests/diferencial.rs`: para cada `corpus/js/*.dart`, a
saída de `node main.mjs` do DartForge é **byte a byte igual** à de `dart
run` (oráculo de semântica) — e o `dartdevc` também é executado no mesmo
arquivo para mostrar, quando falhar, a forma que o oráculo do contrato
emitiu. Depois: pontos de entrada do corpus pub e, por fim, o `new_sali`.
Memória e tempo da emissão medidos no `new_sali` com o mesmo exemplo de
`memoria.rs` das fases anteriores.

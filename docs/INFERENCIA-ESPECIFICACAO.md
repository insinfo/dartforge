# Especificação da inferência de tipos e da análise de fluxo (Dart 3.6 → 3.14)

Contrato normativo do `crates/types` (e, no futuro, da inferência hoje paralela
do `crates/emit_js`). A especificação formal da linguagem
(`references/dart-spec/DartLangSpecDraft.txt`) diz que "assume que a inferência
já ocorreu" e não a especifica; os documentos de `dart-language/resources/
type-system/` são rascunhos defasados em relação ao código. Este documento é
**derivado da implementação** que o nosso oráculo executa.

## 0. Método, autoridade e convenções

### 0.1 Autoridade

Quando as fontes divergem, vale, nesta ordem:

1. **O analyzer 6.11.0 + `_fe_analyzer_shared` 76.0.0** (Dart 3.6.2) — é o que
   o oráculo `tools/oraculo_tipos` executa e o piso da linguagem que
   compilamos (3.6 é o piso, não o teto).
2. A mesma implementação em `references/dart-sdk` (3.14-dev), para o que
   mudou depois de 3.6 — cada regra que mudou traz **as duas versões**,
   marcadas `[3.6]` e `[3.x+]` com a *feature flag* e a versão em que foi
   liberada.
3. O CFE (`pkg/front_end/lib/src/type_inference/`) — mesma semântica, outro
   código; citado quando é mais claro ou quando o analyzer delega a
   `_fe_analyzer_shared`.
4. `dart-language/resources/type-system/*.md` e `accepted/*` — a intenção;
   citados para o nome das regras.
5. `DartLangSpecDraft.txt` — só subtipagem, atribuibilidade e o que a
   especificação formal cobre.

Por quê: o `dart analyze` oficial dá 0 diagnósticos nos projetos-alvo e os
tipos que ele grava são os que o DDC/dart2js usam; a meta é bater com ele
expressão a expressão (docs/FRONTEND-NEW-SALI.md, "Inferência de tipos contra
o oráculo").

### 0.2 Citações

Toda regra traz `arquivo:linha` da implementação, lida:

| prefixo | raiz |
|---|---|
| `an611:` | `%LOCALAPPDATA%/Pub/Cache/hosted/pub.dev/analyzer-6.11.0/` (3.6.2) |
| `fas76:` | `%LOCALAPPDATA%/Pub/Cache/hosted/pub.dev/_fe_analyzer_shared-76.0.0/` (3.6.2) |
| `sdk:` | `references/dart-sdk/` (3.14-dev) |
| `lang:` | `references/dart-language/` |
| `spec:` | `references/dart-spec/DartLangSpecDraft.txt` |

As linhas de `sdk:` valem para o commit de `references/dart-sdk` registrado
em `docs/references-manifest.json`; as de `an611:`/`fas76:` são de pacotes
imutáveis do pub.

### 0.3 Corpus de conformidade (`corpus/inferencia/`)

Cada regra `R-XXX-nn` tem um programa mínimo `corpus/inferencia/<seção>/
<id>_<nome>.dart` e, ao lado, `<id>_<nome>.esperado.tsv` **gravado pelo
oráculo** (nunca escrito à mão):

    python tools/oraculo_tipos/gravar_corpus.py \
        --packages C:/MyDartProjects/new_sali/frontend/.dart_tool/package_config.json

(qualquer `package_config.json` que resolva `analyzer` 6.11.0 e `path`
serve; um único processo `dart` para o corpus inteiro, ~20 s.) Colunas do
`.esperado.tsv`: `linha coluna offset comprimento nó tipo elemento marca
trecho` — `offset`/`comprimento` em unidades UTF-16 (as do analyzer; o nosso
despejo é em bytes UTF-8, `comparar.py` converte), `tipo` é o
`staticType.getDisplayString()` do analyzer, `elemento` o elemento resolvido
(`KIND:dono.nome`). `marca = 1` nas expressões que começam logo depois de um
comentário `/*@*/`: são as que a regra quer verificar; as demais linhas
também são verdade do oráculo e podem ser comparadas. Todos os programas
analisam sem erro no 3.6.2 (`dart analyze corpus/inferencia`); os casos que
são **erro** no 3.6 estão só no texto, com exemplo.

Uso previsto (dono do `crates/types`): o despejo `despejo_tipos` sobre cada
programa comparado com o `.esperado.tsv` por `comparar.py` — uma divergência
num programa mínimo aponta a regra exata.

### 0.4 Notação

* `K` — contexto (esquema de tipo, *type schema*); `_` — o tipo desconhecido
  (`UnknownInferredType`); `T <: S` — subtipo; `UP`/`DOWN` — limites
  superior/inferior padrão (§4); `NonNull(T)` (§9.1); `flatten(T)` (§5.6).
* `X & S` — interseção (variável de tipo promovida); o analyzer a exibe assim.
* "Resultado:" é sempre o que o oráculo grava (`staticType`).

## Sumário

1. Contexto (esquema de tipo), inferência para baixo e para cima
2. Inferência de argumentos de tipo (funções, métodos, construtores)
3. Literais
4. UP e DOWN
5. Expressões de função (closures) e `await`
6. Análise de fluxo — modelo
7. Análise de fluxo — promoção, demoção, captura, laços, try, junções
8. Membros: busca, extensões, `call`, tear-offs, operadores, cascatas
9. Null safety: `!`, `??`, `??=`, encurtamento nulo, Never/Null
10. Padrões
11. Casos de borda dos testes do SDK
12. O que muda de 3.6 a 3.14 (tabela)

## 1. Contexto (esquema de tipo), inferência para baixo e para cima

### 1.1 O tipo desconhecido `_` e os esquemas

* **R-SCH-01.** Um *esquema* é um tipo que pode conter `_` ("ainda não
  fixado"); esquemas nunca aparecem em tipos finais (`inference.md:434-448`).
  Implementação: `UnknownInferredType`, singleton
  (`an611:lib/src/dart/element/type_schema.dart:16-24`). `_?` é `_`
  (`withNullability` devolve `this`, `type_schema.dart:84`), então
  `makeNullable(_) = _` (`an611:lib/src/dart/element/type_system.dart:
  1440-1443`).
* **R-SCH-02.** Esquema *conhecido* = sem `_` em nenhum lugar (argumentos de
  interface, retorno e parâmetros de função, campos de registro)
  (`type_schema.dart:86-115`).
* **R-SCH-03.** Na subtipagem, `_` é topo e fundo **só quando é o tipo
  inteiro** (teste por `identical`, `an611:lib/src/dart/element/subtype.dart:
  41-45`; `sdk:…/subtype.dart:268-271`). Aninhado, `List<_>` é comparado
  estruturalmente e o `_` interno volta a casar com qualquer coisa pelo mesmo
  teste.
* **R-SCH-04 (fechos).** Fecho maior de um esquema: `_` covariante → `Object?`,
  contravariante → `Never`; fecho menor, o inverso
  (`greatestClosureOfSchema`/`leastClosureOfSchema`,
  `an611:…/type_system.dart:616-623`/`1424-1431`; `sdk:…:653`/`1417`;
  `TypeSchemaEliminationVisitor`, `an611:…/type_schema_elimination.dart:
  14-54`). Fecho em relação a variáveis de tipo L (`an611:…/type_system.dart:
  586-600` maior, `1395-1409` menor): o mesmo, e um tipo de função genérico
  cujos limites citam L vira `Function` (maior) / `Never` (menor)
  (`inference.md:519-614`). Ex.: fecho maior de `List<_> Function(_)` =
  `List<Object?> Function(Never)`.
* **R-SCH-05 (contexto `const`).** Antes da inferência para baixo de um
  literal/criação `const`, as variáveis de tipo do contexto são eliminadas
  (`eliminateTypeVariables`, `an611:…/type_system.dart:341-346`, usado em
  `1730-1732`): em `List<T> f<T>() => const [];` o contexto do literal não é
  `List<T>`.

### 1.2 Como o contexto chega a cada expressão

* **R-CTX-00 (entrada única).** Toda expressão é analisada por
  `analyzeExpression(node, esquema)`; **um esquema `dynamic` vira `_` antes do
  despacho** (`fas76:lib/src/type_inference/type_analyzer.dart:557-570`,
  conversão em 560-562; `sdk:pkg/_fe_analyzer_shared/lib/src/type_inference/
  type_analyzer.dart:795-813`, conversão 805-807). Ex.: `dynamic d = [1];` →
  `List<int>`; `dynamic h = 6` → `int` (lit01).
* Despacho: `ResolverVisitor.dispatchExpression` → `resolveExpression(this,
  context)` → `visitXxx(node, {contextType = _})`
  (`an611:lib/src/generated/resolver.dart:751-790`). Filho visitado por
  `visitChildren`/`accept` recebe `_`.
* **Para baixo.** Numa invocação genérica ou literal, o tipo de retorno
  declarado é casado com o contexto (`constrainReturnType`, covariante); a
  solução *preliminar* pode conter `_` e é substituída nos tipos dos
  parâmetros para formar o contexto dos argumentos
  (`an611:lib/src/dart/resolver/invocation_inferrer.dart:219-242`;
  `an611:…/type_system.dart:1702-1737`).
* **Para cima.** O tipo estático de cada argumento é casado com o tipo do
  parâmetro **não substituído** (`constrainArgument`); escolhe-se a solução
  *aterrada* (`invocation_inferrer.dart:576-580`, `274-276`).
* **R-GEN-04 (fixação).** Depois de uma passada preliminar, todo parâmetro de
  tipo cuja solução é totalmente conhecida (e covariante) fica **fixado**; as
  fases seguintes não o mudam e as restrições dele são descartadas
  (`an611:lib/src/dart/element/generic_inferrer.dart:96-118` justificativa,
  `522-535`, `898-899`; `sdk:pkg/analyzer/lib/src/dart/element/
  generic_inferrer.dart:150-165`, `790-793`). **Consequência medida
  (gen04):** `num n = id(1)` → `id<num>`, tipo `num` (não `int`); `num? m =
  id(null)` → `num?`; `List<num> x = [1, 2, 3]` → `List<num>`.

### 1.3 Tabela: forma sintática × contexto passado

K = contexto recebido; "→" = contexto do filho. Citações `an611:` salvo
indicação (RV = `lib/src/generated/resolver.dart`). Linhas verificadas também
em 3.14 quando indicado; o resto é igual em 3.14 salvo §12.

| forma | regra | onde (3.6) | exemplo → tipo (programa) |
|---|---|---|---|
| `var x = e;` local | e → `_` (o elemento é `dynamic` antes, e `dynamic` vira `_`); tipo da variável = tipo de e **rebaixado** (sem `X & S`, `demoteType`); `Null` → `dynamic` | `lib/src/dart/resolver/variable_declaration_resolver.dart:51-65`; RV `1809-1816`; `resolution_visitor.dart:1395-1397` | `var c = [1, 2]` → `List<int>` (ctx01); `var a = null` → `dynamic` (ctx09) |
| `T x = e;` local | e → T | `variable_declaration_resolver.dart:51-55` | `List<num> a = [1, 2]` → `List<num>` (ctx01) |
| `late T x = e;` | igual, entre `lateInitializer_begin/end` | `variable_declaration_resolver.dart:47-49, 70-71` | |
| campo/topo **com tipo** | e → tipo declarado | `variable_declaration_resolver.dart:51-54`; `lib/src/dart/element/element.dart:9034-9036` | |
| campo/topo **sem tipo** (inferência de topo) | inicializador com `_`; `Null` → `dynamic`; ciclo → `dynamic` + TOP_LEVEL_CYCLE; campo cujo tipo veio de *override inference* usa esse tipo como contexto | `lib/src/summary2/ast_resolver.dart:104-118`; `lib/src/summary2/top_level_inference.dart:215-287`; `element.dart:9128-9129` | `var lista = [1, 2.5]` → `List<num>`; `var t = null` → `dynamic` (ctx09, ctx10) |
| *override inference* (membro sem tipo que sobrescreve) | retorno e parâmetros da assinatura combinada dos supertipos (`combineSignatures(doTopMerge: true)`); parâmetro posicional casa por **posição**; sem assinatura combinada → `dynamic` + erro; o corpo usa o tipo herdado como contexto | `lib/src/task/strong_mode.dart:403-483` (retorno 459-465, parâmetros 543-559) | `valor(x) => x` herda `num valor(int)`: `x: int`; `get itens => [1]` → `List<num>`; `m(b, [t])` → `b: int`, `t: String?` (ctx10, ctx11) |
| `x = e` | e → tipo de escrita; para variável local é o tipo **promovido** atual (`promotedType`), senão o declarado | `lib/src/dart/resolver/assignment_expression_resolver.dart:80-87`; `lib/src/dart/resolver/flow_analysis_visitor.dart:1136-1145` | `if (o is List<num>) o = [1]` → `List<num>` (flu19); `d = 1` (`double d`) → `double` (ctx02) |
| `x op= e` | e → tipo do parâmetro do operador, com o refinamento numérico (§8.5) | `assignment_expression_resolver.dart:193-202` | `d += 1` (`double d`): o `1` é `int` (parâmetro `num`), a expressão `double` (ctx02) |
| `x &&= e`, `x \|\|= e` | e → `bool` | `assignment_expression_resolver.dart:190-192` | |
| `x ??= e` | e → tipo de escrita (o mesmo de `=`); tipo = UP(NonNull(T1), T2) com a regra de §4.6 | `assignment_expression_resolver.dart:181-189, 292-324` | `n ??= 1` (`num? n`) → `num`; `l ??= [1]` (`List<num>? l`) → `List<num>` (ctx02) |
| `return e`/`=> e` síncrono | e → tipo de retorno imposto, ou `_` sem ele (retorno declarado `dynamic` não impõe) | RV `3612-3629`; `lib/src/dart/resolver/body_inference_context.dart:177-181`; 3.14 `sdk:pkg/analyzer/lib/src/generated/resolver.dart:5016-5024` | `List<num> f() => [1]` → `List<num>` (ctx03) |
| `return e` em `async` | e → `FutureOr<futureValueType(S)>` (§5.2) | `body_inference_context.dart:209-213`; `type_system.dart:471-498` | `Future<double> u() async => 1` → `double` (ctx03) |
| `yield e` / `yield* e` | e → E de `Iterable<E>`/`Stream<E>` (`asInstanceOf`); `yield*` → `Iterable<E>`/`Stream<E>`. Tipo imposto que não é Iterable/Stream cai em `FutureOr<futureValueType(S)>` | `body_inference_context.dart:183-213`; `yield_statement_resolver.dart:145-168` | `yield [1]` em `Iterable<List<num>>` → `List<num>` (ctx03) |
| argumento | tipo do parâmetro com a substituição **preliminar** (pode conter `_`); nomeado casa pelo nome; chamada dinâmica → `_` | `invocation_inferrer.dart:558-569, 617-629`; RV `3278-3291` | `f([1], y: {1})` → `List<num>`, `Set<Object>` (ctx04) |
| `c ? e1 : e2` | c → `bool`; e1, e2 → K; tipo por §4.6 | RV `2249-2293`; `static_type_analyzer.dart:84-117`; 3.14 `sdk:…/generated/resolver.dart:2810-2858` | `double e = b ? 1 : 2` → o `1` é `double` (ctx05) |
| `e1 ?? e2` | e1 → `K?`; e2 → K, ou **T1** (não NonNull(T1)) se K é `_`/`dynamic`/inválido; tipo por §4.6 | `lib/src/dart/resolver/binary_expression_resolver.dart:153-217`; 3.14 `sdk:…:68-139` | `a ?? []` (`List<num>? a`, sem contexto) → o `[]` é `List<num>` (ctx06) |
| `e!` | e → `K?`; tipo NonNull(T) | `lib/src/dart/resolver/postfix_expression_resolver.dart:189-205`; 3.14 `sdk:…/null_assertion_expression_resolver.dart:37-45` | `int x = g()!` (`T? g<T>()`) → `g<int>()`: `int?` (ctx12) |
| `(e)` | e → K | RV `3345-3355`; `static_type_analyzer.dart:216-220` | `List<num> a = ([1])` → `List<num>` (ctx08) |
| `t..s` (cascata) | t → K; seções → `_`; tipo = tipo de t | RV `2145-2172`; `static_type_analyzer.dart:80-82` | `List<num> b = [1]..add(2.5)` → `List<num>` (ctx08) |
| `await e` | e → K se K é `FutureOr<S>`/`FutureOr<S>?`, senão `FutureOr<K>`; `_` → `FutureOr<_>`; tipo flatten(T) | RV `2073-2085, 4035-4040`; `static_type_analyzer.dart:65-69`; 3.14 `sdk:pkg/_fe_analyzer_shared/…/type_analyzer.dart:493-527` | `double c = await Future.value(1)` → `Future<double>` (ctx07) |
| elemento de lista/conjunto | E da inferência para baixo, senão `_` (§3) | `typed_literal_resolver.dart:95-107` | |
| entrada de mapa `k: v` | k → K_chave, v → K_valor | RV `3127-3158` | |
| `...e` / `...?e` | `Iterable<E>` (lista/conjunto) ou o tipo do mapa; `...?` anulável | RV `3673-3693`; `typed_literal_resolver.dart:104-106, 163-170` | |
| condição de `if` (elemento e instrução), `while`, `do`, `for(;c;)`, `assert` | `bool` | `fas76:…/type_analyzer.dart:723-758`; RV `2430-2448, 3934-3955, 1892-1924`; `for_resolver.dart:211-215` | |
| `if (e case p)`; `switch (e)` instrução | e → `_` | `fas76:…/type_analyzer.dart:604-605, 1912` | |
| `for (x in e)` / `await for` | e → `Iterable<T>`/`Stream<T>`, T = tipo declarado da variável, `_` se sem tipo, ou tipo de escrita do alvo; variável sem tipo = tipo do elemento (`asInstanceOf`) | `for_resolver.dart:98-177` | |
| padrão em declaração/atribuição/for-in | inicializador → esquema do padrão (§10) | `fas76:…/type_analyzer.dart:1503-1511`; RV `3372-3396` | `var (double a, b) = (1, 2)` → o `1` é `double` (pad02) |
| `throw e` | e → **`Object`** (o texto diz `_`) | RV `3790-3800`; 3.14 `sdk:…/generated/resolver.dart:5234-5243` | |
| `e is T`, `e as T` | e → `_` | RV `1852-1869, 3078-3092` | |
| `e[i]` | alvo → `_`; índice → 1º parâmetro de `[]`/`[]=` | RV `2988-3014`; `property_element_resolver.dart:888-907` | |
| `a op b` (operador definível) | a → `_`; b → parâmetro do operador, refinado (§8.5) | `binary_expression_resolver.dart:277-344` | `int i; double d = i + 1` → o `1` é `double` |
| `a == b`, `!=` | ambos `_` | `binary_expression_resolver.dart:92-108` | |
| `a && b`, `a \|\| b`, `!a` | `bool` | `binary_expression_resolver.dart:219-275`; `prefix_expression_resolver.dart:300-305` | |
| `-e`, `~e`, `++`/`--` | operando `_`, **salvo** `-<literal inteiro>`, que repassa K | `prefix_expression_resolver.dart:77-90` | `double i = -7` → `double` (lit01) |
| `${e}` | `_` | RV `3066-3069, 3696-3703` | |
| `switch` expressão | escrutínio `_`; guardas `bool`; braços K; tipo por §4.6 | `fas76:…/type_analyzer.dart:1779-1888` | |
| literal de registro | campo i → campo i de K **só se** K é registro da mesma forma exata; senão `_` em todos | `record_literal_resolver.dart:44-76, 135-190` | `(double, num) c = (1, 2)` → `(double, int)` (lit05) |
| inicializador de campo no construtor `: f = e` | e → tipo do campo | RV `2345-2368` | |
| `super(…)`/`this(…)`, argumentos de constante de enum | argumentos → tipos dos parâmetros | RV `3568-3588, 3706-3725, 2549-2558` | |
| valor padrão `[T p = e]` | e → T | RV `2409-2427` | |
| alvo `f(args)`, `t.m(args)`; instrução-expressão | `_` | RV `2845-2846, 3212, 2624-2629` | |
| declaração de função local | contexto = tipo do elemento; sem retorno escrito é `dynamic`, então nada é imposto (§5.5) | RV `2759-2815`; `function_expression_resolver.dart:163-173` | (clo06) |
| literal de função | §5.1 | `function_expression_resolver.dart:35-125` | (clo01) |

### 1.4 Casos que dão erro no 3.6 (fora do corpus)

* `FutureOr<int Function(int)> c = (x) => x;` — contexto não é tipo de função,
  `x: dynamic`, `invalid_assignment` (§5.1).
* `class D<T extends Comparable<T>> {}` e `var d = D();` / `D x = D();` —
  `could_not_infer` + `type_argument_not_matching_bounds` (o tipo cru
  `D<Comparable<Object?>>`/`D<Comparable<dynamic>>` não é bem limitado).
* `f(C())` com `void f<X extends A<X>>(X x)`, `class C extends B`, `class B
  extends A<B>` — erro em 3.6, `X = B` em 3.7+ (§2.6).

## 2. Inferência de argumentos de tipo (funções, métodos, construtores)

Texto: `inference.md:680-1000` (restrições e solução),
`lang:accepted/2.18/horizontal-inference/feature-specification.md:194-391`.
Implementação: `an611:lib/src/dart/resolver/invocation_inferrer.dart`,
`an611:lib/src/dart/element/generic_inferrer.dart`,
`an611:lib/src/dart/element/type_constraint_gatherer.dart` e
`fas76:lib/src/type_inference/type_analyzer_operations.dart`.

### 2.1 Condutor (`FullInvocationInferrer.resolveInvocation`, 3.6 `invocation_inferrer.dart:157-294`; 3.14 `sdk:…/invocation_inferrer.dart:421-495`)

1. **Argumentos de tipo explícitos** são substituídos; quantidade errada →
   `dynamic` em todos (`:175-216`). Oráculo (gen09): `id<num>(1)` → `num`, o
   `1` fica `int` (contexto `num` aceita `int`).
2. Senão, parâmetros de tipo novos; `setupGenericTypeInference` chama
   `constrainReturnType(retorno declarado, K)` (após `eliminateTypeVariables`
   se `const`) (`:219-238`; `type_system.dart:1702-1737`).
3. **Para baixo**: `choosePreliminaryTypes()` (`:240-241`).
4. **Argumentos** (`:523-584`): **todo** literal de função (sem parênteses)
   é adiado quando `inference-update-1` está ligado (`:548-556`); os demais
   são analisados com o tipo do parâmetro substituído como contexto e
   restringidos com `constrainArgument(tipoArg, tipoParamCru)` (`:558-580`).
5. **Fases** (horizontal, §2.5): antes de cada fase, exceto a primeira, a
   solução preliminar é recalculada (`:252-272`).
6. **Para cima**: `chooseFinalTypes()` (`:274-276`); o tipo de invocação é
   instanciado, parâmetros recalculados e o retorno refinado numericamente
   (`:277-293`, `:631-645`).

Construtores (inclusive fábrica redirecionadora) seguem o mesmo condutor com o
tipo de retorno `C<T…>` (oráculo gen07: `Caixa(1)` → `Caixa<int>`;
`Caixa<num> b = Caixa(1)` → `Caixa<num>` e o `1` fica `int`; `L.de('a')` →
`L<String>`; `L<Object> e = L.de('a')` → `L<Object>`; `Map.of({1: 'a'})` →
`Map<int, String>`).

### 2.2 Geração de restrições `P <# Q [L]` (3.6 `type_constraint_gatherer.dart:131-337`; texto `inference.md:821-995`)

Cláusulas na ordem; a primeira que casa decide.

| # | regra | 3.6 | texto |
|---|---|---|---|
| R-RES-01 | P ou Q é `_`: casa sem restrições | 133-141 | 877-878 |
| R-RES-02 | P é `X ∈ L` não anulável: `_ <: X <: Q` | 143-152 | 879-880 |
| R-RES-03 | Q é `X ∈ L` não anulável: `P <: X <: _` | 154-163 | 881-882 |
| R-RES-04 | P == Q: casa | 165-169 | 883-884 |
| R-RES-05 | Q é `FutureOr<Q0>`: tentar, nesta ordem, FutureOr/FutureOr; `P <# Future<Q0>` com C **não vazio**; `P <# Q0`; `P <# Future<Q0>` com C vazio | `fas76:…/type_analyzer_operations.dart:1056-1100` | 891-898 |
| R-RES-06 | Q é `Q0?`: `P0?`/`Q0`; `dynamic`/`void` contra `Object`; `P <# Q0` com C não vazio; `P <# Null`; `P <# Q0` com C vazio | 180-227 | 899-908 |
| R-RES-07 | P é `FutureOr<P0>`: `Future<P0> <# Q` **e** `P0 <# Q` | 229-244 | 909-911 |
| R-RES-08 | P é `P0?`: `P0 <# Q` **e** `Null <# Q` | 246-262 | 912-914 |
| R-RES-09 | Q é `dynamic`, `void` ou `Object?`: casa | 264-270 | 915-916 |
| R-RES-10 | P é `Never`: casa | 272-275 | 917 |
| R-RES-11 | Q é `Object`: casa só se P não anulável | 277-281 | 918-919 |
| R-RES-12 | P é `Null`: casa só se Q anulável | 283-288 | 920-921 |
| R-RES-13 | P é variável de tipo (ou `X & B`) fora de L: usa o limite (promovido) | 290-300 | 923-927 |
| R-RES-14 | mesma classe: argumentos pela variância | `fas76:…:1118-1158, 1230-1261` | 929-941 |
| R-RES-15 | classes diferentes: `asInstanceOf(P, classe de Q)` e R-RES-14 | `fas76:…:1270-1308` | 943-953 |
| R-RES-16 | Q é `Function` e P função: casa | 309-315 | 955-956 |
| R-RES-17 | funções não genéricas: retorno covariante; posicionais contravariantes (P.obrigatórios ≤ Q.obrigatórios, P.posicionais ≥ Q.posicionais); nomeados na ordem, nomeado obrigatório a mais em P falha | `fas76:…:913-1042` | 958-967 |
| R-RES-18 | funções genéricas: mesmo número de parâmetros de tipo, limites iguais nos dois sentidos, instanciar com Z novos e casar. **[3.6] o fecho das restrições sobre os Z não é feito** (`// TODO(scheglov): do closure`, 449-452); **[3.14]** é (`eliminateTypeParametersInGeneratedConstraints`, `sdk:pkg/_fe_analyzer_shared/…/type_analyzer_operations.dart:1725, 2554-2560`) | 373-455 | 969-985 |
| R-RES-19 | Q é `Record` e P registro: casa | 321-328 | 987-988 |
| R-RES-20 | registros: mesma forma, campo a campo | 463-516 | 990-994 |

Oráculo (gen08): `T f<T>(FutureOr<T> x)`: `f(1)` → `int`;
`f(Future.value(1))` → `int` (R-RES-05, segundo ramo); `f(<int>[])` →
`List<int>`. (gen09) `T n<T>(T? x)`: `n(1)` → `int`; `n(null)` → `Null`;
`n(i)` com `int? i` → `int` (R-RES-06/08).

**[3.14]** (código compartilhado, `sdk:…/type_analyzer_operations.dart:
2196-2439`): com *inference-using-bounds*, R-RES-03 só acrescenta `P <: X`
se X não tem limite ou `P <: fecho maior(limite de X)` (`:2234-2253`); senão
segue para as cláusulas seguintes.

### 2.3 Registro de restrições, fusão e limite como restrição

* Fusão: UP dos limites inferiores, DOWN dos superiores (`inference.md:
  697-708`; `fas76:lib/src/type_inference/type_constraint.dart:137-166`;
  3.6 `generic_inferrer.dart:548-575`; 3.14 `_squashConstraints`,
  `sdk:…/generic_inferrer.dart:720-755`). Oráculo (gen03): `escolha(1, 2.5)`
  → `num`; `escolha(1, 'a')` → `Object`; `escolha(null, 1)` → `int?`;
  `escolha(<int>[], <double>[])` → `List<num>`; (gen10) dois parâmetros
  `void Function(T)` com `(int x){}` e `(num x){}` → `T = int` (DOWN).
* O limite `extends B` vira a restrição `_ <: X <: B[solução atual]`
  (`fas76:…/type_constraint.dart:99-113`; satisfação `:128-135`).

### 2.4 Escolha do tipo (solução)

**Uma variável** (`inference.md:710-723`, aterrada `:765-779`; 3.6
`generic_inferrer.dart:447-491`; 3.14 `sdk:pkg/_fe_analyzer_shared/…/
type_analyzer_operations.dart:1015-1074`). Covariante, nesta ordem: limite
inferior conhecido; limite superior conhecido; limite inferior que não é `_`
(o **fecho menor** dele, se aterrada); limite superior que não é `_` (o
**fecho maior**, se aterrada); `_`.

**Preliminar** (para baixo/horizontal, `_inferTypeParameterFromContext`,
3.6 `:677-735`; 3.14 `:1277-1363`): t = escolha só pelas restrições; se t
contém `_`, devolve t; senão acrescenta a restrição do limite e escolhe de
novo. Ex.: `Object o = math.min(1, 2)` → contexto `T <: Object`, limite
`T <: num`, DOWN = `num`: `T` fixado em `num`.

**Final** (`_inferTypeParameterFromAll`, 3.6 `:632-675`, `toKnownType: true`
em `:672-673`; 3.14 `:1207-1274`): sempre com a restrição do limite.

**Laço por passada** (`_chooseTypes`, 3.6 `:495-546`): parâmetros da
esquerda para a direita, limite substituído pelos já inferidos
(`:508-519`); parâmetro fixado mantém o valor (`:522-524`); na passada
preliminar, resultado conhecido e covariante fica fixado (`:525-535`,
R-GEN-04). **[3.6 → 3.14]** em 3.6 `inferredTypes` começa todo `_`
(`:496-497`): para j > i o limite é substituído com `_` mesmo se Tj foi
fixado antes; em 3.14 começa pelos fixados (`sdk:…:1096-1101`), como o texto
(`inference.md:743-748`), e pula parâmetros sem limite (`:1108`).

**Tipos finais e verificações** (`tryChooseFinalTypes`, 3.6 `:250-387`; 3.14
`sdk:…/generic_inferrer.dart:301-477`):

1. toda restrição, e o limite instanciado, têm de ser satisfeitos; senão
   `COULD_NOT_INFER` **mantendo o tipo errado** (`:260-306`);
2. tipo de função genérico inferido sem *generic-metadata* é erro
   (`:308-329`);
3. os desconhecidos são completados por `instantiateTypeFormalsToBounds`
   (`:331-349`) — `limite ?? dynamic` (§2.7);
4. verificação de limites (`:376-420`);
5. todo resultado é **rebaixado** (sem `X & S`) (`:384`;
   `type_system.dart:322-325`).

Oráculo: (gen01) `id(1)` → `int`; `id([1, 2.5])` → `List<num>`; `id(null)`
→ `Null`. (gen02) `List<T> vazio<T>()`: `List<num> a = vazio()` →
`List<num>`; `var b = vazio()` → `List<dynamic>`; `Iterable<String> c =
vazio()` → `List<String>`; `Object d = vazio()` → `List<dynamic>` (casar
`List<T> <# Object` não gera restrição). (gen06) `f<T extends num>()` sem
contexto → `num`.

### 2.5 Contexto contra o tipo de retorno (para baixo) e soluções parciais

`constrainReturnType(declarado, K)` executa `declarado <# K`
(`generic_inferrer.dart:230-245`; chamado em `type_system.dart:1733-1734`).
Se o casamento **falha**, nenhuma restrição é gravada (`_tryMatchSubtypeOf`
só confirma no sucesso, `:880-908`) e tudo fica `_` (`inference.md:
1513-1514`). Solução preliminar pode ser parcial (`List<_>`), substituída nos
parâmetros como contexto dos argumentos; só as totalmente conhecidas são
fixadas. Ex.: `Map<String, T> m<T>(T x)`; `Map<String, Object> y = m(1)` →
`T` fixado em `Object` antes de ver o argumento: `m<Object>(1)`.

### 2.6 Inferência horizontal (*inference-update-1*, 2.18) (R-GEN-05, R-GEN-12)

* **Todo** argumento que é literal de função é adiado — anotado ou não
  (`invocation_inferrer.dart:548-556`; 3.14 `sdk:…:772-773`); os outros
  ficam na fase 0 (`feature-specification.md:244-258`).
* **Aresta A → B** (A depende de B; texto `:297-326`): A é literal de função
  e existe T livre no tipo de um parâmetro do tipo-parâmetro de A **que A não
  anota**, e livre no **retorno** do tipo-parâmetro de B (ou no tipo inteiro,
  se o de B não é função) (`_FunctionLiteralDependencies`,
  `invocation_inferrer.dart:664-713`; anotação explícita `:24-39`; 3.14
  `sdk:…:1014-1041`).
* **Fases** (`:353-391` do texto): componentes fortemente conexos por
  `DependencyWalker`; fase de um nó = 1 + máx. das dependências; só os
  adiados são emitidos, cada fase em ordem do texto
  (`fas76:lib/src/deferred_function_literal_heuristic.dart:24-60, 81-99,
  121-155`; 3.14 `sdk:…/deferred_function_literal_heuristic.dart:90,
  139-145`).
* **Entre fases** a solução preliminar é recalculada, exceto antes da
  primeira (`invocation_inferrer.dart:252-272`).

Oráculo: (gen05) `aplica(1, (v) => v.isEven)` → `v: int`, `bool`;
`aplicaAntes((v) => v.toDouble(), 1)` (literal **antes** do argumento que fixa
T) → `v: int`, `double`; `l.fold(0, (a, b) => a + b)` → `a: int`, `int`.
(gen12) `g((x) => x.isEven, () => 1)` com `U g<T, U>(U Function(T), T
Function())` → `x: int` (o primeiro literal depende do segundo); mas
`f(() => [], <int>[])` com `T f<T>(T Function(), T)` → **`List<dynamic>`**: o
literal sem dependência vai na primeira fase, antes de qualquer recálculo, e
é inferido com T = `_`; depois `T = UP(List<dynamic>, List<int>)`.

### 2.7 Instanciar para os limites

**Tipo cru `C`** (`DefaultTypesBuilder`, calculado por declaração na ligação;
3.6 `an611:lib/src/summary2/default_types_builder.dart`; texto
`lang:archive/feature-specifications/instantiate-to-bound.md:202-275`):

1. `_breakSelfCycles` (`:137-174`): cadeia de limites `X extends Y, Y extends
   X` → limite `dynamic`; `_breakRawTypeCycles` (`:103-135, 260-351`).
2. `_computeBounds` (`:192-256`): limite inicial = declarado ou `dynamic`
   (`:205-209`); grafo X_i → X_j usados no limite de X_i (`:364-418`); para
   cada componente fortemente conexo (`computeStrongComponents`, `:211-231`)
   substitui por `dynamic` (covariante/invariante) ou `Never`
   (contravariante) (`_UpperLowerReplacementVisitor`, `:420-470`); depois
   substitui cada X_i pelo seu limite em todos (`:233-249`).
3. Resultado em `defaultType`, usado por `instantiateInterfaceToBounds`
   (`type_system.dart:673-683, 741-751, 1827-1834`). 3.14 igual
   (`sdk:pkg/analyzer/lib/src/summary2/default_types_builder.dart`).

Oráculo (gen06, gen13): `class C<T extends num>`: `C()` sem contexto e `C cru
= C()` → `C<num>`; `List l = []` → `List<dynamic>`; `class A<T extends U, U
extends num>`: `A a = A()` → `A<num, num>`; `class O<T extends Object?>`:
`O()` → `O<Object?>` (limite `Object?` escrito **não** é `dynamic`). F-limitado
(`A<X extends A<X>>`) → `A<A<dynamic>>`.

**Completar depois da inferência** (`instantiateTypeFormalsToBounds`, outro
algoritmo, 3.6 `type_system.dart:757-828`; 3.14 `sdk:…:766-826`): conhecidos
ficam; os demais começam em `limite ?? dynamic` (`:775`); resolve-se
repetidamente quem não tem variável livre não resolvida; sem progresso →
erro e `B[não resolvidos := dynamic]` (`:805-823`).

### 2.8 *Inference-using-bounds* (3.7) — **desligado no 3.6**

`fas76:lib/src/experiments/flags.dart:119-123` (`isEnabledByDefault: false`).
Regra (texto `inference.md:737-742, 792-797`; `lang:accepted/3.7/
inference-using-bounds/design-document.md:11-101`): quando Xi tem limite
explícito B e o limite inferior fundido `Mb` não é `_`, gera `C1 = (Mb <# B
[L])` e resolve `C + C1`. Código já presente em 6.11 mas inativo
(`generic_inferrer.dart:654-661, 715-722, 737-792`); 3.14
`sdk:…/type_analyzer_operations.dart:1244-1259, 1331-1347, 1462-1519`.
Exemplo do documento: `f<X extends A<X>>(X x)`, `f(C())` com `C extends B`,
`B extends A<B>` → **[3.6]** `X = C`, erro de limite; **[3.7+]** `X = B`.

### 2.9 Métodos genéricos do SDK (R-GEN-11)

Mesmo condutor. Oráculo (gen11): `l.map((x) => x * 2.5)` →
`Iterable<double>`; `l.expand((x) => [x, x.toString()])` →
`Iterable<Object>`; `l.whereType<num>()` → `Iterable<num>`;
`l.cast<Object>()` → `List<Object>`; `Future.wait([Future.value(1),
Future.value(2)])` → `Future<List<int>>`; `l.reduce((a, b) => a + b)` →
`int`.

## 3. Literais

### 3.1 Inteiro (R-LIT-01)

Tipo `int` se `int` é **atribuível** a K **ou** `double` **não** é
atribuível a K; senão `double` (`an611:lib/src/generated/
static_type_analyzer.dart:180-191`; 3.14 `sdk:…:177-196`). "Atribuível" é
`isAssignableTo` contra o K **cru** (subtipo, ou tear-off de `call`, ou
`dynamic`; `type_system.dart:877-907`), e `_` inteiro aceita tudo
(R-SCH-03). ⚠ O texto (`inference.md:1250-1283`) fala em fecho maior de K;
o resultado coincide nos casos medidos. `-<literal>` repassa K
(`prefix_expression_resolver.dart:79-83`). Faixa: como `double` precisa ser
exato (`INTEGER_LITERAL_IMPRECISE_AS_DOUBLE`), como `int` `int.tryParse`
(`error_verifier.dart:4997-5018`).

Oráculo (lit01): `double a = 1` → `double`; `double? b = 2` → `double`;
`num c = 3` → `int`; `Object d = 4` → `int`; `FutureOr<double> e = 5` →
`double`; `0x10` sem contexto → `int`, com `double` → `double`; `dynamic h =
6` → `int`; `double i = -7` → o prefixo `-7` é `double`. Dentro de argumento
genérico (ctx04): `double d = id(1)` → `id<double>` e o `1` é `double`.

### 3.2 Lista (R-LIT-02)

Inferida como chamada de `List<E> _<E>(E, E, …)`: para baixo
`setupGenericTypeInference(List<E>, K)`; se K não é exatamente `_`, o
contexto dos elementos é `choosePreliminaryTypes()[0]` (pode ser `_`) e
spreads recebem `Iterable<E>`; para cima, cada elemento restringido contra
`E` e `chooseFinalTypes` (`typed_literal_resolver.dart:82-114, 432-493,
628-658`). Tipo contribuído por elemento (`:183-233`): expressão → seu tipo;
`for` → o corpo; `if` → UP dos dois ramos; spread → argumento de
`asInstanceOf(Iterable)`, `dynamic` se dinâmico, `Never` se `Never` ou `...?`
de `Null`. `<T>[]` explícito → `List<T>`.

Oráculo (lit02): `[]` → `List<dynamic>`; `[1, 2.5]` → `List<num>`; `[1,
null]` → `List<int?>`; `<num>[1]` → o `1` é `int`; `List<Object?> e = [1]` →
`List<Object?>`; `List<double> f = [1]` → o `1` é `double`; `[[1], [2.5]]` →
`List<List<num>>` (elementos internos inferidos sem contexto e juntados);
`const [1, 'a']` → `List<Object>`. (nul04) `[null]` → `List<Null>`.

### 3.3 Conjunto/mapa (R-LIT-03, R-LIT-04)

**Passo A — desambiguação** (`_computeSetOrMapResolution`,
`typed_literal_resolver.dart:236-287`), três fontes independentes:
(1) argumentos de tipo: 1 → conjunto, 2 → mapa (`:328-342`); (2) contexto:
`futureOrBase(K)` (tira todos os `FutureOr`, `type_system.dart:436-447`) com
`asInstanceOf` — Iterable e não Map → conjunto; Map e não Iterable → mapa
(`:294-326`); (3) **folhas**: expressões e entradas contadas através de
`if`/`for`; **spreads não são folhas** (`:896-958`). Conflito: prioridade
folhas > argumentos de tipo > contexto (`:259-271`); senão a primeira fonte
inequívoca vence (`:272-281`); **`{}` sem nada é `Map<dynamic, dynamic>`**
(`:282-285`).

**Passo B — para baixo**: `setupGenericTypeInference(Set<E>` ou
`Map<K,V>, contexto)`; preliminares viram contexto de elemento/chave/valor;
spreads de mapa recebem o tipo do mapa (`:116-181`).

**Passo C — para cima** (`_inferSetOrMapLiteralType`, `:516-598`): cada
elemento diz o que pode ser (`:344-430`) — expressão → elemento; entrada →
chave/valor; spread de Iterable → elemento, de Map → chave/valor, de
`dynamic` → os três `dynamic`, de `Never`/`...?Null` → `Never` nos três;
`if` com `else` → UP. Pode/deve ser (`:857-868`): todos podem ser conjunto e
algum deve → `Set`; todos podem ser mapa e algum deve → `Map`; senão o
contexto; senão vazio sem contexto → `Map<dynamic, dynamic>`; senão erro
`AMBIGUOUS_SET_OR_MAP_LITERAL_*` e tipo `dynamic` (`:538-597`).
Texto: `lang:accepted/2.3/unified-collections/feature-specification.md:
85-403`.

Oráculo (lit03, lit04): `{}` → `Map<dynamic, dynamic>`; `{1}` → `Set<int>`;
`{1: 'a'}` → `Map<int, String>`; `Set<int> d = {}` → `Set<int>`;
`Iterable<int> e = {}` → `Set<int>`; `Object f = {}` → `Map<dynamic,
dynamic>`; `{...b}` (b conjunto) → `Set<int>`; `{...c}` (c mapa) → `Map<int,
String>`; `{1: 'a', 2.5: null}` → `Map<num, String?>`; `[...l, ...?n]` →
`List<num>`; `[if (c) 1 else 2.5]` → `List<num>`; `{for (…) i: i.isEven}` →
`Map<int, bool>`; `[for (var i = 0; …) i]` → `List<int>`, `i: int`; `[if (c)
'a']` → `List<String>`; `{...?null, 1}` → `Set<int>`.

**Elementos nulos `?e` (3.8, `null-aware-elements`)** — desligado no 3.6
(`fas76:…/flags.dart:168-172`): contexto interno `E?`, contribuição
NonNull(T) (`lang:accepted/3.8/null-aware-elements/feature-specification.md:
335-486`; 3.14 `sdk:…/typed_literal_resolver.dart:242-243`). ⚠ Em 3.14
`_LeafElements._count` não conta `NullAwareElement` (`sdk:…:1027-1042`);
`{?x}` vira conjunto pelo passo C.

### 3.4 Registros (R-LIT-05)

Contexto campo a campo só com a mesma forma (§1.3); o tipo é o mais
específico, não o contexto (`lang:accepted/3.0/records/
feature-specification.md:516-539`; `record_literal_resolver.dart:44-76,
135-190`). Oráculo (lit05): `(1, 'a')` → `(int, String)`; `(x: 1, y: 2.5)` →
`({int x, double y})`; `(double, num) c = (1, 2)` → `(double, int)`;
`(1, [2], n: null)` → `(int, List<int>, {Null n})`; `a.$1` → `int`; `b.y` →
`double`.

### 3.5 Outros (R-LIT-06)

`double` → `double`; `true`/`false` → `bool`; `null` → `Null`; string,
adjacentes, interpolação → `String`; `#s` → `Symbol`; literal de tipo →
`Type` (`static_type_analyzer.dart:47-49, 73-75, 121-123, 212-214, 230-238,
255-257`; RV `3865-3872`).

## 4. UP e DOWN (limites superior e inferior padrão)

Implementação: `an611:lib/src/dart/element/least_upper_bound.dart`
(`LeastUpperBoundHelper`), `greatest_lower_bound.dart`
(`GreatestLowerBoundHelper`); entrada em `TypeSystemImpl.leastUpperBound`/
`greatestLowerBound` (`an611:lib/src/dart/element/type_system.dart:1434`,
`:626`; `sdk:pkg/analyzer/lib/src/dart/element/type_system.dart:1427-1429`,
`:663-667`). Texto: `lang:resources/type-system/upper-lower-bounds.md:72-337`.
**UP e DOWN nunca normalizam** (NORM, §4.5) — daí a "assimetria" registrada em
`upper-lower-bounds.md:390-415`; o nosso motor não pode normalizar dentro deles.

### 4.1 Predicados auxiliares

| predicado | regra | 3.6 (`an611:…/type_system.dart`) | 3.14 (`sdk:…/type_system.dart`) |
|---|---|---|---|
| TOP(T) | `_`, `dynamic`, tipo inválido, `void`; `S?` se TOP(S) ou OBJECT(S); `FutureOr<S>` se TOP(S) | 1323-1356 | 1319-1352 |
| OBJECT(T) | `Object` (não anulável); `FutureOr<S>` se OBJECT(S) | 1262-1281 | 1259-1277 |
| NULL(T) | `Null`, `Never?` (o analyzer trata `Never?` como `Null`: `an611:…/type.dart:1111-1115`); `S?` se BOTTOM(S) | 1220-1239 | 1218-1236 |
| BOTTOM(T) | `Never`; variável de tipo cuja cadeia de limites acaba em `Never` sem `?` | `an611:…/type.dart:1109`, `1538-1557` | `sdk:…/type.dart:1214`, `1663-1682` |
| MORETOP(T,S) | `void` > `dynamic`/inválido > `Object`; `T?` vs `S?` recursivo; `T` > `S?`; `FutureOr<T>` vs `FutureOr<S>` recursivo | 1121-1191 | 1124-1189 |
| MOREBOTTOM(T,S) | `Never` > `Null`; `T?` vs `S?` recursivo; `T` > `S?`; `X&T` vs `Y&S` pelos limites promovidos; `X&T` > `Y`; `X extends T` vs `Y extends S` pelos limites | 1027-1109 | 1042-1121 |
| anulável(T) | `dynamic`, inválido, `_`, `void`, `Null`, `S?`; `X&B` se B anulável; `FutureOr<S>` se S anulável | 1242-1259 | 1239-1256 |
| não anulável(T) | o dual; **tipo de extensão** só é não anulável se tiver `implements` | 1194-1217 (1209-1210) | 1192-1215 (1207-1208) |

A subtipagem do analyzer trata `_` como topo **e** fundo ao mesmo tempo
(`an611:lib/src/dart/element/subtype.dart:41-44`; `sdk:…/subtype.dart:268-271`)
— várias regras abaixo dependem disso.

### 4.2 UP — casos na ordem exata em que o analyzer os testa

O primeiro caso que casa decide. Linhas de `least_upper_bound.dart`.

| # | regra | 3.6 | 3.14 | exemplo → tipo (programa) |
|---|---|---|---|---|
| R-UP-a | `UP(T,T) = T` (identidade) | 335-338 | 392-395 | `b ? 1 : 2` → `int` |
| R-UP-b | `UP(_,T) = UP(T,_) = T` | 340-346 | 397-403 | (esquemas, §4.4) |
| R-UP-c | ambos TOP: T1 se MORETOP(T1,T2), senão T2 | 348-360 | 405-417 | `Object? o; dynamic d; b ? o : d` → `dynamic` (up02) |
| R-UP-d | TOP(T1) → T1; TOP(T2) → T2 | 362-370 | 419-427 | `b ? 1 : d` → `dynamic` (up02) |
| R-UP-e | ambos BOTTOM: T2 se MOREBOTTOM(T1,T2), senão T1 | 375-384 | 432-441 | |
| R-UP-f | BOTTOM(T1) → T2; BOTTOM(T2) → T1 | 386-394 | 443-451 | `b ? 1 : throw 0` → `int` (up02, nul04) |
| R-UP-g | `UP(X1&B1, T2)`: T2 se `X1 <: T2`; X1 se `T2 <: X1`; senão `UP(B1', T2)` com B1' o fecho maior de B1 em relação a X1 | 396-411 | 453-470 | `if (x is int) b ? x : 1.5` → `num` |
| R-UP-h | `UP(T1, X2&B2)`, simétrico | 413-428 | 470-485 | |
| R-UP-i | ambos NULL: T2 se MOREBOTTOM(T1,T2), senão T1 | 430-442 | 487-499 | `b ? null : null` → `Null` (up02) |
| R-UP-j | NULL(T1): T2 se T2 anulável, senão `T2?`; NULL(T2) simétrico | 450-470 | 504-524 | `b ? 1 : null` → `int?` (up02) |
| R-UP-k | ambos OBJECT: T1 se MORETOP(T1,T2), senão T2 | 472-484 | 526-538 | |
| R-UP-l | OBJECT(T1): T1 se T2 não anulável, senão `T1?`; OBJECT(T2) simétrico | 486-506 | 540-560 | `Object o; b ? 1 : o` → `Object` (up02) |
| R-UP-m | algum lado com `?`: `S?` com S = UP dos dois sem `?` | 508-517 | 562-571 | `b ? 'a' : n` (`int? n`) → `Object?` (up02) |
| R-UP-n | `UP(X1 extends B1, T2)`: T2 se `X1 <: T2`; X1 se `T2 <: X1`; senão `UP(B1', T2)` (limite = `promotedBound ?? bound ?? Object?`) | 522-537 (limite 843-849) | 576-591 (893-899) | `<T extends num>`: `b ? t : 1` → `num`; `U extends T`: `b ? t : u` → `T` (up05) |
| R-UP-o | `UP(T1, X2 extends B2)`, simétrico | 539-555 | 593-609 | |
| R-UP-p | `UP(função, Function) = Function` e o espelho | 557-565 | 611-619 | |
| R-UP-q | duas funções → §4.2.1 | 567-571 | 621-625 | (up03) |
| R-UP-r | `UP(função, T2) = UP(Object, T2)` e o espelho | 573-581 | 627-635 | `b ? f1 : 1` → `Object` (up03) |
| R-UP-s | `UP(registro, Record) = Record` e o espelho | 583-591 | 637-645 | |
| R-UP-t | dois registros → §4.2.2 | 593-596 | 647-650 | (up06) |
| R-UP-u | `UP(registro, T2) = UP(Object, T2)` e o espelho | 598-606 | 652-660 | `b ? (1, 2) : 3` → `Object` (up06) |
| R-UP-v | FutureOr → §4.2.3 | 608-611 | 662-665 | (up07) |
| R-UP-w | o resto são duas interfaces (inclusive tipos de extensão) → §4.2.4 | 613-620 | 667-671 | (up01, up04) |

Notas. (1) Os casos `X&B` vêm antes de NULL, então o resultado nunca é
`(X&B)?` (`upper-lower-bounds.md:7-9`). (2) Depois de R-UP-m nenhum lado tem
`?` (asserção em `an611:…/least_upper_bound.dart:519-520`). (3) [3.14] a
entrada `getLeastUpperBound` ganhou guarda de profundidade: estouro dá
`Object?` (`sdk:…/least_upper_bound.dart:376-389`); 3.6 não tem guarda
(`an611:…:334`).

#### 4.2.1 UP de dois tipos de função (`_functionType`, 3.6 627-738; 3.14 678-794)

1. Número de parâmetros de tipo diferente → `Function` (3.6 631-635); limites
   não iguais (`relateTypeParameters`) → `Function` (637-642); senão os dois
   são instanciados com parâmetros novos.
2. Posicionais percorridos em paralelo: obrigatório com obrigatório, opcional
   com opcional, tipo = **DOWN** dos dois (3.6 656-679); outra combinação
   interrompe.
3. Nomeados, pela ordem dos nomes: mesmo nome → DOWN, obrigatório se for em
   algum lado (3.6 680-693); nome só num lado → descartado se opcional,
   `Function` se obrigatório (694-709).
4. Sobra obrigatória em qualquer lado → `Function` (716-728); sobras
   opcionais são descartadas (prefixo comum).
5. Retorno: UP (730). Tipo do parâmetro: `greatestLowerBound(a, b)` (790-792).

Resultados do oráculo (up03, down01): `int f1(int)` × `double f2(num)` →
`num Function(int)`; aridade diferente → `Function`; `(int)` × `(String)` →
`void Function(Never)`; `({int? x})` × `({String? y})` → `void Function()`;
`((int, num))` × `((num, int))` → `void Function((int, int))`.

#### 4.2.2 UP de dois registros (`_recordType`, 3.6 794-840; 3.14 853-890)

Número diferente de posicionais ou de nomeados → `Record` (3.6 797-805);
posicionais por posição com UP (808-817); nomeados por índice na ordem
ordenada, nome diferente → `Record`, senão UP (820-833). Oráculo (up06):
`(1,'a')`×`(2.5,'b')` → `(num, String)`; `(x: 1)`×`(x: 'a')` →
`({Object x})`; `(x: 1)`×`(y: 1)` → `Record`.

#### 4.2.3 FutureOr (`_futureOr`, 3.6 740-788; 3.14 796-844), nesta ordem

`UP(FutureOr<A>, FutureOr<B>) = FutureOr<UP(A,B)>` (757-761);
`UP(Future<A>, FutureOr<B>)` e o espelho `= FutureOr<UP(A,B)>` (763-773);
`UP(T1, FutureOr<B>) = FutureOr<UP(T1,B)>` e o espelho (775-785); senão cai
na regra de interfaces. Oráculo (up07): `FutureOr<int>`×`int` →
`FutureOr<int>`; ×`Future<int>` → `FutureOr<int>`; ×`2.5` → `FutureOr<num>`;
mas `Future<int>`×`int` → `Object` (sem FutureOr dos lados, é interface).

#### 4.2.4 Interfaces (`InterfaceLeastUpperBoundHelper.compute`, 3.6 49-108; 3.14 48-108)

1. Anulabilidade do resultado: `?` se algum lado tem `?` (186-197); os dois
   são despidos de `?`.
2. `T1 <: T2` → T2; `T2 <: T1` → T1 (56-61).
3. **Mesma classe**: argumento a argumento pela variância do parâmetro —
   covariante UP, contravariante DOWN, invariante exige igualdade mútua
   senão cai no passo 4 (63-101). Oráculo (up04): `<int>[]`×`<double>[]` →
   `List<num>`; `<int>[]`×`<String>[]` → `List<Object>`;
   `Map<String,int>`×`Map<String,double>` → `Map<String, num>`.
4. **Algoritmo clássico** (`_computeLeastUpperBound`, 162-177): conjunto de
   superinterfaces de cada lado (mais o próprio tipo), interseção, e o único
   tipo de profundidade máxima (`_computeTypeAtMaxUniqueDepth`, 285-315);
   se a profundidade máxima empata, desce até haver um só. Profundidade
   (`_computeLongestInheritancePathToObject`, 204-281): `Object` e `Null` = 1,
   `Object?` = 0, tipo de extensão sem `implements` = 1, mixin = máx. das
   restrições + 1, classe = máx.(superclasse, interfaces, mixins) + 1.
   `_addSuperinterfaces` (119-155): tipo de extensão sempre acrescenta
   `Object?` mais os `implements`. **[3.14]** aplicação de mixin *nomeada*
   (`class A = B with M;`) subtrai 1 (`sdk:…:311-316`); 3.6 não.
   Oráculo (up01): `B implements A`×`C implements A` → `A`;
   `X implements I1, I2`×`Y implements I1, I2` → `Object` (empate na
   profundidade 2); `<int>[]`×`<int>{}` → `Iterable<int>` (up04);
   tipos de extensão (up09): `E<int>`×`E<double>` → `E<num>`;
   `E<int>`×`int` → `Object?` (sem `implements`); `F implements int`,
   `F(1)`×`2` → `int`.

### 4.3 DOWN — casos na ordem (`greatest_lower_bound.dart`)

| # | regra | 3.6 | 3.14 |
|---|---|---|---|
| R-DOWN-a | `DOWN(T,T) = T` | 29-32 | 28-31 |
| R-DOWN-b | `DOWN(_,T) = DOWN(T,_) = T` | 34-40 | 33-39 |
| R-DOWN-c | ambos TOP: T1 se MORETOP(**T2**,T1), senão T2 (o *menos* topo) | 45-54 | 44-53 |
| R-DOWN-d | TOP(T1) → T2; TOP(T2) → T1 | 56-64 | 55-63 |
| R-DOWN-e | ambos BOTTOM: T1 se MOREBOTTOM(T1,T2), senão T2 | 69-78 | 68-77 |
| R-DOWN-f | BOTTOM(T1) → T1; BOTTOM(T2) → T2 | 80-88 | 79-87 |
| R-DOWN-g | ambos NULL: T1 se MOREBOTTOM(T1,T2), senão T2 | 93-102 | 92-101 |
| R-DOWN-h | `DOWN(Null, T2)`: `Null` se `Null <: T2`, senão `Never` (e simétrico) | 110-130 | 106-126 |
| R-DOWN-i | ambos OBJECT: T1 se MORETOP(T2,T1), senão T2 | 135-144 | 131-140 |
| R-DOWN-j | OBJECT(T1): T2 se não anulável; senão NonNull(T2) se não anulável; senão `Never` (e simétrico) | 146-178 | 142-174 |
| R-DOWN-k | `DOWN(T1?,T2?) = S?`; `DOWN(T1?,T2) = DOWN(T1,T2?) = S`, S = DOWN sem `?` | 180-193 | 176-189 |
| R-DOWN-l | duas funções (antes dos testes de subtipo): tipos de parâmetro de tipo iguais senão `Never`; posicionais pareados sem olhar obrigatoriedade, tipo **UP**, opcional se for em algum lado; nomeados de mesmo nome UP, obrigatório só se nos dois; nome só num lado entra opcional; sobras entram opcionais; retorno DOWN | 198-200 (+259-377) | 194-196 (+255-370) |
| R-DOWN-m | dois registros: mesma forma senão `Never`; campo a campo DOWN | 202-204 | 198-200 |
| R-DOWN-n | `T1 <: T2` → T1; `T2 <: T1` → T2 | 206-214 | 202-210 |
| R-DOWN-o | `DOWN(FutureOr<A>, FutureOr<B>) = FutureOr<DOWN(A,B)>`; `DOWN(FutureOr<A>, Future<B>) = Future<DOWN(A,B)>`; `DOWN(FutureOr<A>, T2) = DOWN(A,T2)` (e espelhos) | 216-249 | 212-245 |
| R-DOWN-p | senão `Never` | 251-252 | 247-248 |

**[3.6 → 3.14]** o teste de subtipo de R-DOWN-n: 3.6 usa `isSubtypeOf` cru
(`an611:…:207, 212`); 3.14 compara os **fechos maiores em relação a `_`**
(`sdk:…:203, 208, 372-380`), como pede `inference.md:624-626`.

Oráculo (down01, gen10): `f<T>(void Function(T), void Function(T))` com
`(int x){}` e `(num x){}` → `T = int` (DOWN dos limites superiores).

### 4.4 UP/DOWN de esquemas com `_`

Texto (`inference.md:605-626`): `UP(T,_) = DOWN(T,_) = T`; dentro de UP os
testes `<:` usam os fechos menores, dentro de DOWN os maiores. Analyzer:

* `_` no topo: R-UP-b e R-DOWN-b.
* `_` aninhado em UP (3.6 e 3.14) e em DOWN [3.6]: nenhum fecho é calculado;
  como a subtipagem trata `_` como topo e fundo, `UP(List<_>, List<int>)`
  testa `List<_> <: List<int>` = verdadeiro e dá `List<int>`.
* Fechos (`greatestClosureOfSchema`/`leastClosureOfSchema`,
  `an611:…/type_system.dart:616`/`1424`): `TypeSchemaEliminationVisitor`
  troca `_` covariante por `Object?` (maior) / `Never` (menor) e o inverso em
  posição contravariante (`an611:…/type_schema_elimination.dart:32-34`).
  Fecho em relação a variáveis de tipo: `LeastGreatestClosureHelper`
  (`sdk:…/least_greatest_closure.dart:12-96`) — tipo de função genérico cujos
  limites citam a variável eliminada vira `Never` (menor) ou `Function`
  (maior).
* O CFE segue o texto à risca (`sdk:pkg/front_end/lib/src/type_inference/
  standard_bounds.dart:11-46`).

### 4.5 NORM (normalização) — `an611:lib/src/dart/element/normalize.dart`

| regra | 3.6 | 3.14 |
|---|---|---|
| primitivos (`dynamic`, inválido, `Never`, `void`, interface não anulável sem argumentos) ficam | 101-113 | 109-118 |
| `FutureOr<T>`: S = NORM(T); TOP(S) → S; `Object` → S; `Never` → `Future<Never>`; `Null` → `Future<Null>?`; senão `FutureOr<S>` | 61-99 | 66-104 |
| `T?`: S = NORM(T); TOP(S) → S; `Never` → `Null`; `Null` → `Null`; `FutureOr<R>` com R anulável → S; senão `S?` | 166-200 | 168-203 |
| `X & T`: S = NORM(T); `Never` → `Never`; TOP(S) → `X`; S é X → `X`; S é `Object` e NORM(limite) é `Object` → `X`; senão `X & S` | 204-, 240- | 207-216, 241-279 |
| `C<T0..Tn>` → `C<NORM(Ti)>`; registros e funções campo a campo | ~136-142 | 140-165 |

Divergência do texto (`normalization.md:91-97` diz "se NORM(B) <: S então
X"): o analyzer só implementa o caso `S = Object`. Onde o analyzer
normaliza: igualdade de tipos em tempo de execução, fusão de superinterfaces
(`an611:…/class_hierarchy.dart:183,187`), combinação de assinaturas de membros
(`an611:…/inheritance_manager3.dart:1118`), igualdade de constantes, e o
`operations.normalize` da análise de fluxo (consistência das variáveis de
padrões-ou). **Não** em UP/DOWN nem no tipo estático de expressões.

### 4.6 Onde UP é usado com contexto: *inference-update-3*

A *flag* `inference-update-3` foi liberada em **3.4**
(`an611:lib/src/dart/analysis/experiments.g.dart:308-316`), portanto já vale
no 3.6 para bibliotecas de versão ≥ 3.4. Regra comum (R-UP-08): T = UP dos
ramos; S = fecho maior do contexto K; se `T <: S`, o tipo é T; senão, se
**cada** ramo é `<: S`, o tipo é **S**; senão T.

| construção | regra | 3.6 | 3.14 |
|---|---|---|---|
| `c ? e1 : e2` | e1 e e2 com contexto K | `an611:lib/src/generated/static_type_analyzer.dart:84-117` | `sdk:pkg/analyzer/lib/src/generated/static_type_analyzer.dart:74-112` |
| `e1 ?? e2` | e1 com `K?`; e2 com K, ou com T1 se K é `_`/dynamic; T = UP(NonNull(T1), T2) | `an611:…/resolver/binary_expression_resolver.dart:153-217` | `sdk:…/binary_expression_resolver.dart:68-140` |
| `v ??= e` | e com o tipo de leitura T1; T = UP(NonNull(T1), T2) | `an611:…/assignment_expression_resolver.dart:293-327` | `sdk:…:891-928` |
| `switch` expressão | sem casos → `Never`; braços com K; UP em dobra | `fas76:lib/src/type_inference/type_analyzer.dart:1797-1894` | `sdk:pkg/_fe_analyzer_shared/…/type_analyzer.dart:2383-2501` |
| elementos de coleção | restrição `Ti <: E` por elemento, juntadas por UP; **sem** a regra do contexto | `an611:…/typed_literal_resolver.dart:183-199` | `sdk:…:195-247` |

Oráculo (up08): `C1 implements B1, B2`, `C2 implements B1, B2`: sem contexto
`b ? C1() : C2()` → `A` (única de profundidade máxima); com contexto `B1` →
`B1`; `B2 x = n ?? C2()` → `B2`; `switch` com contexto `B1` → `B1`.
(Em 3.14 o código compartilhado passa `topType: dynamic` ao fecho do
`switch` — contorno da issue 4466 que só o CFE usa; o analyzer segue com
`Object?`.)

## 5. Expressões de função (closures) e `await`

Texto: `lang:resources/type-system/inference.md:263-372` (a entrada de
2024-12-17 do changelog, `inference.md:9-12`, é a mudança de `return;` em
geradores). Implementação: `an611:lib/src/dart/resolver/
function_expression_resolver.dart` e `an611:lib/src/dart/resolver/
body_inference_context.dart` (3.14: `sdk:pkg/analyzer/lib/src/dart/resolver/`
mesmos nomes).

### 5.1 Parâmetros vindos do contexto (R-CLO-01)

* **O contexto só é usado se for um tipo de função** (`contextType is
  FunctionType`, `an611:…/function_expression_resolver.dart:35`;
  `sdk:…:46`). `F?` ainda é tipo de função. `Function`, `FutureOr<F>`,
  `Object`, `_` ou qualquer outro contexto → nenhum parâmetro inferido e
  nenhum tipo de retorno imposto. O CFE faz o mesmo
  (`sdk:pkg/front_end/lib/src/type_inference/inference_visitor_base.dart:
  2261-2262`).
  Consequência medida: `FutureOr<int Function(int)> c = (x) => x;` é **erro**
  no 3.6 (`x` fica `dynamic`, o literal é `dynamic Function(dynamic)`,
  `invalid_assignment`) — por isso o corpus usa `(int x) => x`.
* O número de parâmetros de tipo tem de bater; o contexto é instanciado com os
  parâmetros de tipo do próprio literal (`_matchTypeParameters`, 3.6 132-151).
* Parâmetro sem tipo recebe K = o tipo do parâmetro do contexto na **mesma
  posição** (posicionais) ou de **mesmo nome** (nomeados) (3.6 102-124); o
  tipo é o fecho maior de K; se `T <: Null`, `Object?`; `dynamic` fica
  `dynamic` implícito (3.6 81-100).
* Sem contexto (ou sem o parâmetro correspondente): `dynamic`.
* Tipo de retorno imposto = retorno do contexto, **salvo `dynamic` ou `_`**,
  que não impõem nada (3.6 45-48).

Oráculo (clo01): `void Function(int) f = (x) {…}` → `x: int`;
`var h = (x) => x` → `dynamic Function(dynamic)`; `Function k = (x) => 1` →
`int Function(dynamic)`; `Object o = (x) => 1` → `int Function(dynamic)`;
`[1].map((x) => x)` → `int Function(int)`.

### 5.2 Contexto do `return`/`=>`/`yield` (`_contextTypeForImposed`, 3.6 168-214; 3.14 214-262) (R-CTX-03)

Com tipo imposto S:

* nem `async` nem gerador: S;
* `sync*`: E tal que S casa com `Iterable<E>`; `async*`: E de `Stream<E>`.
  **[3.14]** o casamento é feito com `unionFreeType(S)` (tira `?` e
  `FutureOr` recursivamente, `sdk:…/type_system.dart:1804-1823`); [3.6] com
  `asInstanceOf` sobre S (3.6 186-207);
* `async`: `FutureOr<futureValueType(S)>` (3.6 209-213), com
  `futureValueType`: `S?` → recursivo; `Future<S>`/`FutureOr<S>` → S;
  `dynamic` → `dynamic`; `void` → `void`; senão `Object?`
  (`an611:…/type_system.dart:471-498`).
* `yield* e`: `Iterable<ctx>`/`Stream<ctx>`, e `_` sem contexto
  (`an611:lib/src/dart/resolver/yield_statement_resolver.dart:145-161`).
* Sem tipo imposto: `_` (`an611:lib/src/generated/resolver.dart:2594-2616`).

O mesmo vale para funções e métodos declarados, com o tipo de retorno escrito
como S. Oráculo (ctx03): `List<num> f() => [1]` → `List<num>`;
`Future<List<num>> g() async => [1]` → `List<num>`; `yield [1]` em
`Iterable<List<num>>`/`Stream<List<num>>` → `List<num>`;
`Future<double> u() async => 1` → o literal é `double`.

### 5.3 Tipo realmente retornado (`_computeActualReturnedType`, 3.6 144-158; 3.14 183-207) (R-CLO-02)

* **Não gerador**: começa em `Null` se o fim do corpo é alcançável, senão
  `Never` (corpo `=>` faz `handleReturn` antes, então começa em `Never`);
  dobra UP sobre cada `return e` (tipo achatado por `flatten` se `async`,
  `addReturnExpression`, 3.6 60-70); **`return;` acrescenta `Null`**.
* **Gerador**: sem `yield` nenhum → **`dynamic`**; senão UP dos `yield`
  (`yield* e` contribui o argumento de `Iterable`/`Stream`, 3.6 72-89).
* **[3.6] `return;` em gerador acrescenta `Null`** (3.6 60-62, sem teste de
  gerador); **[3.14]** é ignorado (`sdk:…/body_inference_context.dart:97-103`,
  a mudança de 2024-12-17). Oráculo (clo04):
  `() sync* { yield 1; return; }` → **`Iterable<int?> Function()`** no 3.6
  (em 3.14 seria `Iterable<int>`); `() sync* {}` → `Iterable<dynamic>`.

### 5.4 Ajuste ao contexto e embrulho (`_clampToContextType` + `computeInferredReturnType`, 3.6 91-142; 3.14 132-181) (R-CLO-03/05)

R = o contexto do corpo. Se R é `void`, ou a função é `async` e R é
`FutureOr<void>` → retorno `void`. Senão, se `T <: R` → T; senão R. Sem
contexto → T. Embrulho: `async` → `Future<flatten(S)>`; `async*` →
`Stream<S>`; `sync*` → `Iterable<S>`; senão S. Guardado como o tipo de
retorno do literal (`dynamic` se nada foi calculado; `_resolve2`, 3.6
153-161).

Oráculo (clo02, clo03, clo05): `() => 1` → `int Function()`;
`if (b) return 1; return 2.5;` → `num Function()`; `() {}` → `Null
Function()`; `if (b) return 1;` (fim alcançável) → `int? Function()`;
`() => throw 0` e `() { throw 0; }` → `Never Function()`; `() async {}` →
`Future<Null> Function()`; `() async => Future.value(1)` → `Future<int>
Function()`; `Future<num> Function() d = () async => 1` → **`Future<int>
Function()`** (T <: R, fica T); `void Function() a = () => 1` → `void
Function()`; `FutureOr<int> Function() b = () => 1` → `int Function()`.

### 5.5 Funções locais (R-CLO-06)

Função local sem tipo de retorno escrito é inferida como literal de função
sem contexto (o tipo de retorno do elemento é o calculado em 5.3/5.4). Oráculo
(clo06): `f() => 1` → `int Function()`; `g(int x) { return x * 2.5; }` →
`double Function(int)`.

### 5.6 `flatten`, tipo futuro e `await` (R-CLO-07, R-CTX-07)

`flatten(T)` (`an611:…/type_system.dart:388-434`; `sdk:…:425-471`):
`_` → `_`; `S?` → `flatten(S)?`; `X & S` → `flatten(U)` se S tem tipo futuro
U, senão `flatten(X)`; se o **tipo futuro** de T é `Future<S>`/`FutureOr<S>`
→ S, e `S?` se for `Future<S>?`/`FutureOr<S>?`; senão T.

Tipo futuro (`futureType`, 3.6 452-464; `_futureTypeOfBounded`, 3.6 1846;
3.14 487-499, 1874-1906): se T é não anulável e implementa `Future<U>`, é
esse; senão, se T é "limitado por S" (o próprio T, ou variável de tipo — o
limite declarado antes do promovido — cuja cadeia de limites acaba em S) e S
é `FutureOr<U>`, `Future<U>?` ou `FutureOr<U>?`, é S.

`await e` com contexto K: o operando recebe `FutureOr<K>` (K se K já é
`FutureOr<…>`; `dynamic` vira `_` antes) (`an611:lib/src/generated/
resolver.dart:2073-2085, 4035-4040`; 3.14 compartilhado em
`sdk:pkg/_fe_analyzer_shared/lib/src/type_inference/type_analyzer.dart:
493-547`, que também aceita `FutureOr<S>?`). Tipo: `flatten(T)`
(`an611:lib/src/generated/static_type_analyzer.dart:65-69`).

Oráculo (clo07, ctx07): `await (FutureOr<int>)` → `int`; `await
(Future<int>?)` → `int?`; `T extends Future<int>`, `await t` → `int`;
`await (Future<Future<int>>)` → `Future<int>` (um nível só); `await 1` →
`int`; `List<num> a = await Future.value([1])` → `Future<List<num>>`;
`double c = await Future.value(1)` → `Future<double>`.

## 6. Análise de fluxo — modelo

Implementação: `fas76:lib/src/flow_analysis/flow_analysis.dart` (3.6; abaixo
**F76:n**) e `sdk:pkg/_fe_analyzer_shared/lib/src/flow_analysis/
flow_analysis.dart` (3.14; **FA:n**); cola do analyzer em
`an611:lib/src/dart/resolver/flow_analysis_visitor.dart` (**AFV76:n**) e
`sdk:pkg/analyzer/lib/src/dart/resolver/flow_analysis_visitor.dart`
(**AFV:n**); promovibilidade de campos em `sdk:pkg/_fe_analyzer_shared/lib/src/field_promotability.dart` (**FP:n**, igual em 3.6 salvo formatação). Texto: `lang:resources/type-system/flow-analysis.md` (**LANG:n**)
— **atrás da implementação** na junção de cadeias (§7.9).

### 6.1 *Flags* que mexem no fluxo

| flag | liberada | o que muda | 3.6 |
|---|---|---|---|
| `non-nullable` | 2.12 | análise completa (bibliotecas antigas: `FlowAnalysis.legacy`) | ligada (AFV76:101, 272-284) |
| `constructor-tearoffs` | 2.15 | com ela, o inicializador de `var` implícita cria versão de valor e estado de condição (sem ela, o bug #1785 é mantido) | ligada (FA:8814-8821; F76:5957-5963) |
| `patterns` | 3.0 | constantes de `case` promovem; casos inalcançáveis | ligada (FA:6505-6522) |
| `inference-update-2` | 3.2 | promoção de campo final privado | ligada (F76:5925-5927; AFV76:104-105) |
| `sound-flow-analysis` | **3.9** | assume null safety sólida (§7.18) | **ausente** |
| `inference-update-4` | não liberada | `final` pula a junção conservadora; `x = e` devolve referência a `x` | desligada |
| `this-promotion`, `promotion-chain-intersection-join`, `anonymous-methods` | não liberadas | `this` promovível; junção por subsequência comum; blocos anônimos | ausentes |

Fonte das versões: `sdk:tools/experimental_features.yaml:161-522`;
`an611:lib/src/dart/analysis/experiments.g.dart:319-326`. Mudanças só no
CHANGELOG: 3.3 (#54056) getter abstrato não impede promoção
(`sdk:CHANGELOG.md:2920-2928`); 3.7 (#56893) campo promovido a `Null` afeta
alcance (`:1886-1890`); 3.13 (#62889) "mudança menor para evitar
insolidez" (`:265-266`, sem detalhe).

### 6.2 Estado

* **FlowModel(alcançável, promoções)** imutável; cada atualização devolve um
  novo (F76:2059-2072; FA:3626-3646). O mapa de promoções é persistente e
  **sem noção de escopo** (variáveis fora de escopo continuam nele).
* **PromotionModel** por chave (F76:2977-3038; FA:4529-4596; LANG:190-223):
  `promotedTypes` (cadeia de promoção, cada um subtipo do anterior; [3.6]
  `null` = sem promoção, [3.14] lista vazia), `tested` (tipos de interesse),
  `assigned`/`unassigned` (atribuição definitiva), `ssaNode` [3.6] /
  `version` [3.14] (identidade do valor atual; `null` = **capturada para
  escrita**, F76:3038), histórico de não promoção. Invariantes: não ambos
  atribuída e não atribuída; capturada nunca promovida (FA:4570-4581).
* O tipo declarado não é guardado: vem de `operations.variableType`.
  `declare(v, inicializada)` cria modelo novo com versão nova (FA:3717-3728).
* **Alcance**: árvore de pontos de controle; `overallReachable =
  locallyReachable && pai.overallReachable` (F76:3664; FA:5381-5421);
  `split`/`unsplit`/`setUnreachable` (FA:5449-5476).
* **Versões de valor (SSA)**: nova a cada escrita e a cada junção de versões
  diferentes (F76:3778-3805; FA:5497-5536). Cada versão guarda o *estado de
  condição* do valor escrito (§7.10) e as chaves das propriedades: uma chave
  estável por nome para propriedade promovível, chave nova a cada acesso para
  a não promovível (F76:3813-3836). `this` e `super` têm versões próprias.
* **ExpressionInfo(tipo, ifTrue, ifFalse)**: trivial quando `ifTrue` e
  `ifFalse` são o mesmo; `_invert` troca (FA:120-178). Subclasses: referência
  (chave de promoção + versão), referência de propriedade, `_NullInfo` (o
  literal `null`). [3.6] a informação fica presa à última `Expression`
  visitada e é consumida uma vez (`_storeExpressionInfo`/
  `_getExpressionInfo`, F76:4285-4300, 5691-5747); [3.14] é devolvida
  explicitamente. Expressão sem informação = trivial do estado atual.

## 7. Análise de fluxo — promoção, demoção, captura, laços, try, junções

### 7.1 Primitivas

* **R-FLU-P1 `tryPromoteForTypeCheck(ref, T)`** (F76:2536-2572; FA:4000-4067):
  capturada → trivial. `ifTrue`: S1 = `tryPromoteToType(T, atual)`; se passo
  válido, `T` entra em `tested` e S1 na cadeia. `ifFalse`: `factor(atual, T)`;
  fator fundo → sem promoção ([3.6] ainda alcançável; [3.9+] ramo
  **inalcançável**); senão promove ao fator; `T` sempre entra em `tested`.
* **R-FLU-P2 `tryPromoteToType(para, de)`** (`an611:lib/src/dart/element/
  type_system.dart:1751-1782`; `sdk:…:1771-1802`): `para <: de` → `para`;
  senão, se `de` é variável de tipo e `para <: limite(de)` → **`X & para`**
  (anulabilidade: `X?` com `para` não anulável dá não anulável,
  `sdk:…:2135-2152`); senão nenhuma. Limite = `promotedBound ?? bound ??
  dynamic` (`sdk:…/element/type.dart:1656-1657`).
* **R-FLU-P3 `tryMarkNonNullable(ref)`** (F76:2476-2498): novo = NonNull(anterior)
  se passo válido; **não** entra em `tested`; `ifFalse` inalterado (nunca
  promove a `Null`).
* **R-FLU-P4 `tryPromoteForTypeCast`** (F76:2508-2525): um modelo só, regra do
  `ifTrue`; `T` entra em `tested`.
* **Passo válido** [3.6]: `novo != anterior` (F76:2487, 2518, 2549); [3.9+]
  "não subtipo mútuo" (FA:7039-7057). Ex.: `Object? o; if (o is dynamic) o`
  promove a `dynamic` em 3.6, não em 3.9+.
* **factor(T, S)** (`an611:…/type_system.dart:351-383`; LANG:600-610): `T <:
  S` → `Never`; `R?` → `factor(R,S)` se `Null <: S`, senão `factor(R,S)?`;
  `FutureOr<R>` → `factor(R,S)` se `Future<R> <: S`, `factor(Future<R>,S)` se
  `R <: S`; senão T. Oráculo (flu20): `int? x; if (x is int) {} else x` →
  **`Never?`** (fator de `int?` por `int`, exibido sem normalizar);
  `FutureOr<int> fo; if (fo is int) {} else fo` → `Future<int>`.
* **NonNull** (`an611:…/type_system.dart:1500-1531`; §9.1).

### 7.2 `is` / `is!` (R-FLU-01)

`isExpression_end` (F76:4889-4903; FA:6988-7020): `is Never` (F76:4891,
`isNever`) → como literal booleano; senão, operando referência →
R-FLU-P1 (invertido para `is!`); [3.9+] tipo estático já `<: T` num
não-referência → `true`. Texto LANG:711-732.

Oráculo (flu01): `if (o is int) o` → `int`; depois do `if` → `Object`;
`if (o is! String) return; o` → `String`; `num n; if (n is String) n` →
`num` (String não é subtipo de num: sem promoção).

### 7.3 Variáveis de tipo e interseção `X & S` (R-FLU-13)

Criada por `is`/`as` quando S é subtipo do limite mas não de X (R-FLU-P2),
por NonNull de X (`X & NonNull(limite)`, `X & Object` sem limite) e pela
inicialização de `var` implícita com valor `X & S` (FA:8840-8848). Acesso a
membro usa `resolveToBound`, que prefere o limite promovido
(`sdk:…/type_system.dart:1681-1704`); exibição `X & S`, entre parênteses com
sufixo (`(X & S)?`, `sdk:…/display_string_builder.dart:350-367`). Rebaixada
na demoção por atribuição, no `var` inferido (`demoteType`,
`an611:…/type_system.dart:322-325`; tipo **declarado** da variável é X, a
variável começa promovida a `X & S`) e nos resultados da inferência genérica.

Oráculo (flu13, flu06, nul01): `f<T, U extends num?>`: `if (t is int) t` →
`T & int`, `t.isEven` → `bool`; `if (u != null) u` → `U & num`; `if (u is
int) u` → `U & int`; `if (t != null) t` → `T & Object`; `if (t is int) { var
d = t; d }` → `T & int`; `t!` (T sem limite) → `T & Object`.

### 7.4 `==`/`!=` com `null`, `identical` (R-FLU-02)

`_equalityCheck` (F76:5658-5689; FA:8451-8483) e `equalityOperation_end`
(F76:4597-4639): ambos `Null` → resultado conhecido; um lado o **literal**
`null` (`_NullInfo`; parênteses transparentes) → R-FLU-P3 no outro lado
(invertido para `==`); o resto → sem informação. **Uma variável de tipo
`Null` não conta** (LANG:702-706). [3.9+] `Null` contra não anulável →
"sabidamente diferente". `identical(a, b)` passa pelo mesmo caminho
(`sdk:pkg/analyzer/lib/src/dart/resolver/invocation_inferrer.dart:666-684`).

Oráculo (flu02): `if (x != null) x` → `int`; `if (x == null) return; x` →
`int`; `if (null != y) y else y` → `int` / `int?`; `if (z == null) z` →
`int?` (nunca promove a `Null`).

### 7.5 `== true`, propriedades públicas, `this` (R-FLU-17)

Literal booleano não é `_NullInfo`: `b == true` não dá informação
(FA:8480-8481). `this` **não é promovível** em 3.6 (uma chave só, sempre
`ThisNotPromoted`, F76:5609-5619, 6127-6134) nem por padrão em 3.14
(`this-promotion` não liberada, FA:6226-6235). Oráculo (flu17): `bool? b; if
(b == true) b` → `bool?`; `if (p.v != null) p.v` (campo público) → `int?`;
`if (this is Q) this` → `P`.

### 7.6 `as` e `!` (R-FLU-04)

`asExpression_end` → R-FLU-P4 (F76:4353-4357); `nonNullAssert_end` →
NonNull (F76:5040-5047). Oráculo (flu04): `o as String; o` → `String`; `x!;
x` → `int`; `o is int ? o : o` → `int` / `Object`.

### 7.7 `&&`, `||`, `!`, condicional (R-FLU-03)

`&&`: `true(N) = true(E2)`, `false(N) = junção(false(E1), false(E2))`; `||`
o dual (F76:4955-4985); `!` inverte; `c ? a : b`: `true(N) =
junção(true(a), true(b))`, idem `false` (FA:6461-6489). Texto LANG:739-776.
Oráculo (flu03): `o is int && o.isEven` → `int` à direita; `o is! int ||
o.isEven` → `int`; `if (!(o is String)) return; o` → `String`; `x == null ||
x.isEven` → `int`; **variável de condição**: `var t = x != null && …; if (t)
x` → `int` (§7.10).

### 7.8 Atribuição: demoção e tipos de interesse (R-FLU-05, R-FLU-06, R-FLU-16, R-FLU-19)

`write` (F76:3084-3134; FA:4657-4723; LANG:492-566):

1. Capturada → nada promovido, só `assigned`.
2. **Demoção**: percorre a cadeia do fim e mantém o maior prefixo cujo último
   elemento é supertipo do tipo escrito (`_demoteViaAssignment`,
   FA:4743-4780).
3. **Promoção a tipo de interesse** (`_tryPromoteToTypeOfInterest`,
   F76:3207-3309): candidatos = `NonNull(declarado)` (se diferente do
   declarado) e cada `T` testado e `NonNull(T)`; candidato C serve se `escrito
   <: C`, `C <: atual` e passo válido; igualdade exata ao tipo escrito vence;
   senão o único subtipo de todos os outros; senão nenhum. [3.14] também exige
   `C <: declarado` e tipo escrito não inválido.
4. **Demoção total** (cadeia vai de não vazia a vazia): [3.6] **limpa
   `tested`** (F76:3120-3125); [3.9+] mantém (issue #4380).

Declarações (`_initialize`, F76:5947-5978): escrita com promoção a tipo de
interesse só se **tipo escrito e não `final`**
(`promoteToTypeOfInterest = !isImplicitlyTyped && !isFinal`, FA:8837);
`var` implícita com valor de variável de tipo é promovida a esse tipo.
`var x;` sem inicializador é não atribuída e tem tipo `dynamic`; atribuir
não a promove (`dynamic` não tem tipo de interesse).

Oráculo (flu05, flu06, flu16, flu19): `if (o is int) { o = 'a'; o }` →
`Object`; `if (o is int) {} o = 1; o` → `int` (int foi testado); `num n; n =
1.5; n` → `num` (double não é de interesse); `int? x; x = 1; x` → `int`
(NonNull(int?)); `x = null; x` → `int?`; `var a; a = 1; a` → `dynamic`; `num b
= 1; b` → `num`; `int? c = 1; c` → **`int`**; `int e; e = 2; e` → `int`;
`x ??= 1; x` → `int`; `var z = y ?? (throw 0)` → `z: int` e depois `y:
int`.

O contexto do lado direito de `x = e` é o tipo **promovido** atual
(`an611:lib/src/dart/resolver/flow_analysis_visitor.dart:1136-1145`, via
`assignment_expression_resolver.dart:80-87`): `if (o is List<num>) { o =
[1]; }` → o literal é `List<num>` e `o` segue `List<num>` (flu19).

### 7.9 Junções (R-FLU-11)

`join` (FA:4195-4273, 4953-4997): um lado inalcançável → o outro; senão, por
chave presente nos dois: cadeia por `joinPromotedTypes`; `tested` união;
`assigned` e `unassigned` E lógico; capturada OU lógico (e limpa `tested`);
versão nova se diferem. Chave só num lado cai (LANG:328-338).

**Junção de cadeias** — padrão em 3.6 e 3.14 (`_legacyJoinPromotedTypes`,
F76:3385-3419; FA:5240-5277): percorre as duas; iguais → mantém e avança as
duas; `t2 <: t1` → pula t1; `t1 <: t2` → pula t2; sem relação → **para**; se
uma cadeia foi consumida sem pulos, é ela; senão o coletado. A junção por
maior subsequência comum que o texto descreve (LANG:166-186) só existe com a
*flag* experimental `promotion-chain-intersection-join` (FA:5019-5098).
Ex.: `A`, `C` sem relação, ambos supertipos de `B`: `if (c) { o as A; o as
B; } else { o as C; o as B; } o` → tipo declarado (padrão); `B` com a
*flag*.

Oráculo (flu11): `case 0: if (x == null) return; x` → `int`; `case 1: x` →
`int?`; `for … { if (x == null) break fora; x }` → `int`; os dois ramos de
`if/else` com `return` → `int` depois.

### 7.10 Variáveis de condição

A escrita/inicialização guarda na versão nova a `ExpressionInfo` não trivial
do lado direito; a leitura a restaura (`rebaseForward`), salvo se a variável
foi capturada ou escrita depois (FA:5849-5934, 8078-8107; [3.6]
`addPreviousInfo`, F76:4080-4111). `late` nunca guarda (FA:8810-8813).
Oráculo (flu03): `var t = x != null && …; if (t) x` → `int`.

### 7.11 Closures e captura (R-FLU-07)

Pré-passe (`sdk:pkg/_fe_analyzer_shared/lib/src/type_inference/
assigned_variables.dart:108-131, 324-347`): por nó, variáveis lidas,
escritas, capturadas para leitura/escrita e declaradas; escritas dentro de
closure/inicializador `late` contam como **capturadas** no nó que o contém.
`_functionExpression_begin` (F76:5708-5722; FA:8485-8507):

1. na função externa, as variáveis escritas **dentro** do literal passam a
   capturadas **daqui em diante** (`conservativeJoin(∅, escritas)`);
2. dentro do literal, toda variável escrita **em qualquer lugar** do membro
   perde as promoções, e toda capturada em qualquer lugar é capturada;
3. no fim, restaura o modelo externo do passo 1.

`conservativeJoin` (F76:2256-2286): escritas perdem promoções e a não
atribuição, ganham versão nova; capturadas ficam capturadas. Inicializador
`late` é tratado como closure (FA:7075-7093).

Oráculo (flu07): `x` nunca escrito → `int` dentro do literal; `y` escrito
dentro de um literal → `if (y != null) y` → `int?` (capturada); `z` escrito
**depois** do literal → `int?` dentro dele (regra 2).

[3.14, sem *flag*] **suspensão**: depois de `await`/`yield` numa função
local, variáveis lidas por ela, escritas em qualquer lugar e não declaradas
nela são demovidas (FA:7709-7745; motivo `DemoteViaSuspension`). Não existe
em 3.6.

### 7.12 Laços, saltos e `try` (R-FLU-08, R-FLU-09, R-FLU-10)

* `while` (FA:8110-8144): `antes(cond) = conservativeJoin(split, escritas,
  capturadas)` do laço; `depois = inheritTested(unsplit(junção(false(cond),
  breaks)), depois(corpo))`. `for` igual (sem condição = `true`,
  AFV:307-320; `continue` junta antes dos atualizadores). `do`
  (FA:6591-6621): `conservativeJoin` no corpo; condição junta os `continue`;
  `depois = junção(false(E), breaks)`. `for-in` (FA:6753-6772):
  `conservativeJoin` antes do corpo; `depois = junção(depois(corpo),
  antes(corpo))`. `break`/`continue` juntam no alvo e tornam o caminho
  inalcançável (FA:6799-6821). [3.6] F76:4563-4717, 5570-5594. Texto
  LANG:828-891.
* **Saídas** (FA:6823-6839; F76:4757-4760): `throw`, `rethrow`, `return` e
  qualquer expressão de tipo estático fundo (`Never`) tornam o caminho
  inalcançável (`fas76:lib/src/type_inference/type_analyzer.dart:566-567`);
  literal booleano torna o ramo oposto inalcançável.
* `try/catch` (FA:7943-8003): cada `catch` começa de
  `conservativeJoin(antes do try, escritas(corpo), capturadas(corpo))`;
  variáveis de exceção e pilha declaradas atribuídas (tipos: o do `on T`,
  senão `Object`; pilha `StackTrace`).
* `try/finally` (FA:8010-8068): `antes(finally) = junção(depois do try,
  conservativeJoin(antes do try, …))`; `depois = attachFinally(depois do try,
  antes do finally, depois do finally)`. Em `attachFinally` (FA:8203-8413):
  variável possivelmente mudada no `finally` (capturada ou versão diferente)
  → fica o estado do `finally`; senão [3.6] `rebasePromotedTypes(base =
  finally, novo = try)` (F76:2090-2234, a ordem antiga, issue #4382); [3.9+]
  `rebase(try, finally)` (FA:8367-8380). `rebasePromotedTypes`:
  F76:3466-3495; LANG:351-372.

Oráculo (flu08, flu09, flu10): `x` atribuído dentro do `while` → `int?` já
no início do corpo; `y` não atribuído no `while`/`for-in` → `int`; `y`
atribuído no `do` → `int?`; no `try` que escreve `y`: `x` (não escrito) →
`int` no `catch`, `y` → `int?` no `catch` e no `finally`; `catch (e)` → `e:
Object`; `on FormatException catch (e, s)` → `FormatException`,
`StackTrace`; `try {…} finally { o as int; } o` → `int` (flu20);
`if (x == null) throw 0; x` → `int`; `if (y == null) falha(); y`
(`Never falha()`) → `int`.

### 7.13 Campos finais privados (3.2, `inference-update-2`) (R-FLU-12)

`_handleProperty` (F76:5921-5945; FA:8763-8797): promovível se o membro
existe, a *flag* está ligada e `isPropertyPromotable(membro)` — no analyzer
`field.isPromotable` de um `PropertyAccessorElement` de `FieldElement`
(AFV76:577-582). `isPromotable` é calculado por biblioteca
(`sdk:pkg/_fe_analyzer_shared/lib/src/field_promotability.dart:63-304`;
`sdk:pkg/analyzer/lib/src/summary2/library_builder.dart:866-960`;
[3.6] `an611:lib/src/summary2/library_builder.dart:1519-1525`):

1. nome privado (FP:200-204);
2. campo não final ou `external` de mesmo nome em qualquer lugar da
   biblioteca → não promovível (FP:213-220);
3. getter **concreto** de mesmo nome → não promovível; abstrato não (3.3,
   FP:241-259);
4. classe concreta da biblioteca cuja interface tem o nome sem implementá-lo
   (encaminhador de `noSuchMethod`) → não promovível (FP:281-300);
5. métodos não contam; campo de representação privado de tipo de extensão é
   sempre promovível; mixins contam como abstratos, enums como concretos.

Alvo: receptor explícito → versão do receptor; `_f`/`this._f` implícito →
versão de `this`; `super._f` → versão própria. A promoção dura enquanto a
versão do alvo não muda; escrever a variável-alvo cria versão nova e perde as
promoções de campo; variável capturada ganha versão nova a cada leitura, então
os campos dela nunca ficam promovidos.

Oráculo (flu12): `final int? _x`: `if (_x != null) _x` → `int`;
`this._x` → `int`; `outro._x` (outro objeto) → `int`; `final int? pub` →
`int?`; `int? _mutavel` → `int?`.

### 7.14 Padrões e `switch` (R-FLU-14)

`if-case` e `switch` promovem o **escrutínio** (FA:6853-6861, 7883-7888);
declaração/atribuição de padrão e `for-in` com padrão não. `promoteForPattern`
(F76:5207-5274): tipo casado não anulável → conhecido vira NonNull; promove o
cache casado e o escrutínio correspondente (quando "denota o valor casado":
propriedade, ou variável cuja versão não mudou, FA:9143-9168); caminho não
casado juntado com o `ifFalse` quando o padrão não cobre o tipo. `p?`:
como `!= null` ([3.6] sempre acrescenta o caminho não casado, F76:5100-5122);
`p!`: sempre casa e promove. `== null`/`!= null` constantes passam por
`_handleEqualityCheckPattern` (F76:5848-5919). `switch`: cada caso contra o
modelo "não casado" dos anteriores (FA:7797-7842); rótulo em caso →
`conservativeJoin`; no fim, `default` implícito se não exaustivo; sem `break`
nenhum, o que vem depois é inalcançável. Variáveis que dividem um corpo de
caso são fundidas e sempre atribuídas.

Oráculo (flu14): `if (o case int i)` → `i: int` e `o: int`; `if (x case var
v?)` → `v: int`, `x: int`; `switch (o) { case String s: … }` → `s`, `o`:
`String`; `if (x case != null) x` → `int`.

### 7.15 `?.` e `??` no fluxo (R-FLU-15, R-FLU-16)

`_nullAwareAccess_rightBegin` (F76:5049-5068; FA:8952-9010): `split`, guarda o
caminho de atalho, promove o alvo a não nulo **dentro da cadeia** (alvo de
tipo `Null` → caminho não nulo inalcançável); `nullAwareAccess_end` junta com
o atalho. **A promoção dura só até o fim da cadeia**; a condição `a?.v !=
null` **não** promove `a` em 3.6 (a referência do alvo é destruída antes de
`sound-flow-analysis`, FA:8978-8987). `??` (F76:4798-4828): o atalho
(esquerda não nula) promove a referência da esquerda; esquerda `Null` →
atalho inalcançável; `end` junta. `??=`: `ifNullExpression_rightBegin(lhs)`,
lado direito, `write(lhs)`, `end` (`an611:…/assignment_expression_resolver.
dart:90-92`).

Oráculo (flu15, flu16): `if (a?.v != null) a` → `A?`; `if (b?.w != null) b`
(`w` não anulável) → `A?`; `x ??= 1; x` → `int`.

### 7.16 `late` e atribuição definitiva (R-FLU-18)

`_checkReadOfNotAssignedLocalVariable` (`sdk:pkg/analyzer/lib/src/generated/
resolver.dart:5597-5637`): `late` → erro só se **definitivamente não
atribuída**; `final` não definitivamente atribuída →
`readPotentiallyUnassignedFinal`; potencialmente não anulável não
definitivamente atribuída → `notAssignedPotentiallyNonNullableLocalVariable`;
anulável pode. Escrita em `late final` definitivamente atribuída e em `final`
não definitivamente não atribuída → erro
(`sdk:…/assignment_expression_resolver.dart:1265-1290`). O tipo lido é o
declarado (promovido, se for o caso). Oráculo (flu18): `late int x; if (b) x =
1; x` → `int`; `late final y = [1.5]` → `List<double>`; `int z` atribuída nos
dois ramos → `int`; `late int topo` → `int`.

### 7.17 "Por que não promoveu" (NonPromotionReason)

| classe | quando | 3.6/3.14 |
|---|---|---|
| `DemoteViaExplicitWrite(variável, nó)` | escrita removeu a promoção | F76:33; FA:54-84, 9254 |
| `DemoteViaSuspension` | [3.14] `await`/`yield` em função local | FA:86-118 |
| `PropertyNotPromotedForInherentReason` | getter/tear-off, nome público, `external`, não final | FA:5303-5334 |
| `PropertyNotPromotedForNonInherentReason` | conflito de nome (campo não promovível, getter concreto, encaminhador de `noSuchMethod`) ou *flag* desligada | FA:5353-5371, 4318-4386 |
| `ThisNotPromoted` | `this` testado | F76:4036, 5808-5818; FA:5807-5819 |

Cálculo: para propriedades percorre as versões não promovíveis anteriores;
para variáveis, as entradas do histórico cujo tipo não é supertipo do atual
(FA:8522-8617). Texto das mensagens no analyzer: AFV76:766-787;
`sdk:pkg/analyzer/lib/src/generated/resolver.dart:6774-6984`.

### 7.18 `sound-flow-analysis` (3.9) — o que muda em relação ao 3.6

| # | 3.9+ | 3.6 | onde |
|---|---|---|---|
| 1 | passo inválido se subtipo mútuo | inválido só se igual | FA:7039-7057; F76:2549 |
| 2 | `is T` com fator fundo: ramo falso inalcançável | alcançável, sem promoção | FA:4034-4060; F76:2557-2560 |
| 3 | `Null` × não anulável em `==`: sabidamente diferente | sem informação | FA:8459-8469; F76:5667-5677 |
| 4 | `is`/`as` que falham pela anulabilidade: `false` / inalcançável | nada | FA:6282-6287, 6994-6999 |
| 5 | `e is T` em não-referência com tipo `<: T`: `true` | nada | FA:7011-7015 |
| 6 | `??` com esquerda não anulável: direita inalcançável | alcançável | FA:6907-6911; F76:4809-4812 |
| 7 | `?.` em alvo não anulável: atalho inalcançável; promoção de campo através de `?.` | alcançável; sem | FA:8970-8987 |
| 8 | `p?` em valor não anulável sempre casa; padrão que não pode casar pela anulabilidade: inalcançável | pode não casar | FA:8743-8749, 7533-7558; F76:5102-5111 |
| 9 | demoção total mantém `tested` | limpa | FA:4703-4713; F76:3120-3125 |
| 10 | `try/finally`: `rebase(try, finally)` | `rebase(base = finally, novo = try)` | FA:8367-8380; F76:2178-2181 |

Ex. (3.9+): `String y; if (i != null) y = 'a'; y` (`int i`) é válido; em 3.6
é erro "y precisa ser atribuída" (`sdk:tools/experimental_features.yaml:
258-264`).

## 8. Membros: busca, extensões, `call`, tear-offs, operadores, cascatas

Implementação: `an611:lib/src/dart/resolver/` (`type_property_resolver.dart`,
`method_invocation_resolver.dart`, `property_element_resolver.dart`,
`extension_member_resolver.dart`, `applicable_extensions.dart`,
`function_reference_resolver.dart`, `binary_expression_resolver.dart`,
`prefix_expression_resolver.dart`, `postfix_expression_resolver.dart`,
`assignment_expression_resolver.dart`) e
`an611:lib/src/dart/element/inheritance_manager3.dart`. Em 3.14 vários
resolvedores foram divididos (`increment_or_decrement_resolver.dart`,
`unary_operator_invocation_resolver.dart`, `logical_not_resolver.dart`,
`null_assertion_expression_resolver.dart`) com as mesmas regras de tipo;
onde só há citação `sdk:` a linha de 3.6 não foi conferida uma a uma, mas a
regra é a mesma (as diferenças estão em §12).

### 8.1 O despachante: `TypePropertyResolver.resolve` (R-MEM-01..04)

Todo acesso a membro de instância (leitura, escrita, chamada, operador,
padrão relacional, getter de padrão de objeto) passa por aqui
(`an611:…/type_property_resolver.dart:62-231`; `sdk:…:64-243`), nesta ordem:

| passo | condição | resultado | 3.6 |
|---|---|---|---|
| 1 | nome `new` | erro | 75-79 |
| 2 | receptor limitado por `dynamic` (ou inválido) | busca **só em `Object`**, sem erro para nome desconhecido (resultado `dynamic`) | 81-90 |
| 3 | anulabilidade: tipo de extensão só é anulável se escrito `V?`; os outros por `isPotentiallyNullable` | | 92-97 |
| 4 | receptor **anulável**: (a) membros de `Object`; (b) senão extensões aplicáveis ao tipo anulável; (c) senão erro `UNCHECKED_*_OF_NULLABLE_VALUE` e recuperação por `resolveToBound` | | 99-177 |
| 5 | não anulável: `resolveToBound(receptor)` | | 179 |
| 5a | interface | `_lookupInterfaceType`; **membro da interface vence extensão** | 181-185 |
| 5b | `Function` e nome `call` | sem erro, sem elemento | 186-191 |
| 5c | tipo de função e nome `call` | o próprio tipo de função | 194-201 |
| 5d | `Never` | busca em `Object`, erros suprimidos | 203-208 |
| 5e | registro | `fieldByName` → campo | 210-220 |
| 5f | nada ainda | extensões sobre o tipo **não resolvido** | 222-225 |
| 5g | nada ainda | membros de `Object` | 227 |

Regra do nome-base: se há o getter mas se pede o setter (ou o contrário),
extensões **não** são consultadas ([3.6] `_lookupInterfaceType` sempre busca
os dois nomes, `an611:…:251-268`; texto `lang:accepted/2.7/
static-extension-methods/feature-specification.md:256`).

**Interface (InheritanceManager3).** [3.6] `getMember(InterfaceType, …)`
sobre `getMember2(InterfaceElement, …)` (`an611:…/inheritance_manager3.dart:
278-298, 311-338`); [3.14] os nomes trocaram (`getMember3` sobre
`getMember`, `sdk:…:223-288`). A interface guarda os membros declarados e,
para cada outro nome, a assinatura mais específica herdada. Construção
(`sdk:…:547-744`): superclasse, depois cada mixin (o membro do mixin
**substitui** — `class X extends S with M1, M2` é a cadeia `S&M1`, `S&M2`),
depois as interfaces; declarados vencem; nome não declarado →
`_findMostSpecificFromNamedCandidates` (conflito getter/método é erro) →
**assinatura combinada** (`_combineSignaturesImpl`, `sdk:…:433-471`): os
candidatos cujo tipo é subtipo do tipo de **todos** os outros; nenhum →
conflito; um (ou vários do mesmo tipo) → ele; senão `NNBD_TOP_MERGE` dos
tipos de função normalizados ([3.6] `an611:…:94-108`, `topMerge` em `:1120`;
texto `lang:accepted/2.12/nnbd/feature-specification.md:930-947`).
`super.m` lê `superImplemented.last[nome]` (nulo para tipos de extensão;
`an611:…:319-333`).

Oráculo (mem01): `d.valor()` (sobrescrito com retorno `int`) → `int`, elemento
`METHOD:D.valor`; `d.mm()` do mixin → `String`, `METHOD:M.mm`;
`J implements I1 (num get v), I2 (int get v)`: `j.v` → **`int`**, elemento
`GETTER:I2.v` (assinatura combinada).

**Receptor anulável** (texto `nnbd/feature-specification.md:540-550,
785-793`). Oráculo (mem02): `int? x`: `x.toString()` → `String`
(`Object.toString`); `x.hashCode` → `int`; `x?.isEven` → `bool?`;
`x.runtimeType` → `Type`; `x == 1` → `bool` (elemento `num.==`).

**Receptor `dynamic`** (`spec:DartLangSpecDraft.txt:9334-9336,
14689-14728`): resultado `dynamic`, salvo membros de `Object`: `d.hashCode` →
`int`, `d.runtimeType` → `Type`, `d.toString()` → `String` **só se** o número
de posicionais bate e não há nomeados (`_hasMatchingObjectMethod`,
`an611:…/method_invocation_resolver.dart:197-201, 450-487`); `d == 1` →
`bool`. Oráculo (mem03): `d.foo()`, `d.bar`, `d[0]`, `d + 1` → `dynamic`;
`d.toString()` → `String`; `d.hashCode` → `int`; `d.runtimeType` → `Type`;
`d == 1` → `bool`.

**Receptor `Never`** (texto `nnbd/feature-specification.md:596-604,
799-803`): chamada → `Never` (`an611:…/method_invocation_resolver.dart:
532-549`); operador binário com esquerda `Never` → `Never`, **inclusive
`==`**. Oráculo (mem03): `falha()` → `Never`; mas `falha().x` (propriedade
inexistente, código morto) → **`InvalidType`** no 6.11 — não usar `Never`
aqui para bater com o oráculo.

**Variável de tipo** (`resolveToBound`, `sdk:…/type_system.dart:1681-1704`:
`X & S` → S; `X` sem limite → `Object?`; senão o limite). Oráculo (mem04):
`T extends List<int>`: `t.first` → `int`; `t.map((e) => e * 2.5)` →
`Iterable<double>`; `t[0]` → `int`; `U` sem limite: `u.toString()` →
`String`.

**Tipo de função e `Function`**: `f.call(a)` tipa como `f(a)`; `f.call` → o
tipo de função; `Function g; g.call(1)` → `dynamic`
(`an611:…/method_invocation_resolver.dart:813-834`). **Registros**: `$1…$n`
e getter por nome (`lang:accepted/3.0/records/feature-specification.md:
397-403`); `r.$1(x)` chama o valor do campo (`an611:…:836-839`).

### 8.2 Extensões (R-EXT-01..04)

**Aplicabilidade** (texto `static-extension-methods/feature-specification.md:
251-260`; `nnbd/…:908-917`): acesso de instância; o tipo não tem membro com
o nome-base (`dynamic` "tem todos"; `Never`/`void` nunca têm extensão);
`findExtension` = acessíveis → com o membro → `applicableTo(receptor)`
(`sdk:…/extension_member_resolver.dart:93-104`). `applicableTo`
(`an611:…/applicable_extensions.dart:186-245`; `sdk:…:205-281`): vazio para
`Never`; para cada extensão, parâmetros de tipo novos e `GenericInferrer` com
`constrainArgument(receptor, on-type cru)`; `tryChooseFinalTypes()` nulo →
não aplicável; depois exige **`receptor <: on-type instanciado`**. Extensão
`on int` não se aplica a `int?`.

**Especificidade** (texto `:262-274`; `an611:…/extension_member_resolver.
dart:382-414`; `sdk:…:465-503`): E1 mais específica que E2 se E2 é de
biblioteca da plataforma e E1 não; ou, ambas (ou nenhuma) da plataforma,
`T1 <: T2` (on-types instanciados) e (não `T2 <: T1`, ou os on-types
instanciados para os limites estão estritamente ordenados). Torneio com
conjunto de ambíguas (`sdk:…:346-387`); ambiguidade → erro
(`AMBIGUOUS_EXTENSION_MEMBER_ACCESS` em 3.6).

**Override explícito** `E(x).m` / `E<T>(x).m` (texto `:99-120, 169-215`;
`an611:…/extension_member_resolver.dart:60-80, 142-209`): inferido como o
construtor `E(this.$target)` sem contexto; `E(e)?.m` promove `e` a não nulo
antes e torna o resultado anulável; fora de alvo de acesso é erro (tipo
`dynamic`). Membros de extensão **não** recebem o refinamento numérico
(`sdk:…/type_system.dart:1918-1924, 2020-2026`).

**`call`, operadores, getters/setters de extensão**: mesmo caminho; `e(args)`
usa o `call` de extensão se o tipo não tem `call`; **o tear-off implícito não
usa `call` de extensão** (só membros da interface,
`sdk:pkg/analyzer/lib/src/generated/error_detection_helpers.dart:315-347`);
a instanciação explícita `e<T>` usa (`an611:…/function_reference_resolver.
dart:152-170`).

**Tipos de extensão (3.3)** (`lang:accepted/3.3/extension-types/
feature-specification.md:535-670`; `sdk:…/inheritance_manager3.dart:746-778`):
membros da declaração e das superinterfaces tipo de extensão; membros de
`Object` sempre pelo caminho de não extensão; tipo de extensão é anulável só
se escrito `V?`; exaustividade usa o apagamento.

Oráculo: (ext01) `'a'.dobro` → `int` (extensão sem nome, elemento
`GETTER:null.dobro`); `[1].primeiro` (`E<T> on List<T>`) → `int`;
`[1.5].soma()` (`N<T extends num> on Iterable<T>`) → `double`;
`[1].mapa((x) => '$x')` → `List<String>`. (ext02) `A on Iterable<int>` e `B on
List<int>`: `[1].q` → `int` (B, mais específica); `A([1]).q` → `String`;
`X on String { String get length }`: `'a'.length` → `int` (interface vence);
`X('a').length` → `String`. (ext03) `on int?`: `x.vazio` → `bool`; operador
`'abc' - 1` → `String`; `'abc'()` (`call` de extensão) → `int`. (ext04)
`extension type Id(int v)`: `Id(1)` → `Id`; `i.proximo()` → `Id`; `i.v` →
`int`; `Nome('a').length` (`implements String`) → `int`; `i as int` →
`int`.

### 8.3 `call` (R-MEM-05, R-MEM-11)

| forma | regra | onde |
|---|---|---|
| `o(args)`, `o` interface com método `call` | como `o.call(args)` | `spec:DartLangSpecDraft.txt:9357-9365` |
| `f.call(args)` em tipo de função; em `Function` | o tipo de função; `dynamic` | `an611:…/method_invocation_resolver.dart:813-834` |
| tear-off implícito `F f = o;` | se o contexto **aceita tipo de função** (`F`, `Function` ou `FutureOr` deles, `an611:…/type_system.dart:126-131`), `o` é não anulável (variáveis de tipo pelo limite) e a **interface** tem `call`: vira `o.call` (`ImplicitCallReference`); `call` genérico é instanciado pelo contexto; não em alvo de cascata, ramo de `?:` ou operando de `??` | `an611:lib/src/generated/resolver.dart:4085-4146, 4195-4221`; `an611:lib/src/generated/error_detection_helpers.dart:312-334` |
| explícito `o<T>` | `o.call<T>` (interface ou extensão) | `lang:accepted/2.15/constructor-tearoffs/feature-specification.md:362-364` |

Nota do oráculo (mem05): o nó `ImplicitCallReference` não aparece no despejo
(o visitante genérico não o visita); o identificador `f` em `int
Function(String) g = f` fica com tipo `F`. `f('a')` →
`FunctionExpressionInvocation` `int`; `f.call` → `int Function(String)`.
(mem11) `T f<T>(T Function() g)`, `f(C())` com `C.call() → int` → `T = int`
(o tear-off implícito entra na inferência como `int Function()`).

### 8.4 Invocação genérica, tear-offs e instanciação (R-MEM-06)

* Invocação `o.m<…>(args)`: o condutor de §2.1.
* **Instanciação implícita pelo contexto** (`insertGenericFunctionInstantiation`,
  `an611:lib/src/generated/resolver.dart:1171-1224`;
  `an611:…/type_system.dart:635-670`; `an611:…/generic_inferrer.dart:199-226`):
  tipo estático **genérico** de função e `flatten(K)` tipo de função **não
  genérico** → casa o tipo sem os parâmetros de tipo com K e `chooseFinalTypes`;
  embrulha num `FunctionReference`. Aplicado depois de identificadores,
  acesso a propriedade, invocações, `as`, `await`, `=`, binárias, índice e
  literais de função (RV 1867-3658).
* **Explícita** `f<int>` (`an611:…/function_reference_resolver.dart:39-150,
  225-285`): contagem errada → `dynamic`.
* **Tear-off de construtor** (`an611:…/constructor_reference_resolver.dart:
  88-146`; `invocation_inference_helper.dart:126-149`): classe genérica sem
  argumentos → instanciada só se K é tipo de função; senão fica genérico.

Oráculo (mem06, gen01): `int Function(int) f = id` → `FunctionReference`
`int Function(int)` (o `SimpleIdentifier` interno guarda `T Function<T>(T)`);
`id<String>` → `String Function(String)`; `var h = id` → `T Function<T>(T)`;
`C.new` → `C<T> Function<T>()`; `C<int>.nomeado` → `C<int> Function()`;
`C<num> Function() m = C.new` → `C<num> Function()`; `[1].map<String>` →
`Iterable<String> Function(String Function(int))`. **Em toda chamada o
identificador da função genérica guarda o tipo genérico** (`id` em `id(1)` →
`T Function<T>(T)`), não o instanciado. (mem11) `S Function<S>(S) g = id` é
válido (subtipagem de funções genéricas com renomeação dos parâmetros de
tipo).

### 8.5 Operadores (R-MEM-07..10)

* **Binário** = chamada do operador do operando esquerdo
  (`spec:DartLangSpecDraft.txt:10838-10845`); esquerda com contexto `_`,
  direita com o parâmetro refinado; tipo (`an611:…/binary_expression_resolver.
  dart:461-488`): esquerda `Never` → `Never`; `==` → `bool`; esquerda
  dinâmica sem elemento → `dynamic`; senão o retorno do operador, refinado.
* **Refinamento numérico** (`+ - * % remainder`, `clamp`; não para membros de
  extensão) — contexto do operando direito (`an611:…/type_system.dart:
  1880-1935`): `int` se `int <: C`, `num` não `<: C` e `T <: int?`; senão
  `double` se `double <: C`, `num` não `<: C` e `T` não `<: double?`; senão
  `num`. Tipo (`an611:…/type_system.dart:1982-2034`): `T <: double?` →
  `double`; `S <: double?` (S não fundo) → `double`; `T, S <: int?` → `int`;
  senão `num`; `clamp`: tudo `int` → `int`, tudo `double` → `double`, senão
  `num`. ⚠ O texto dá `D` para `D extends double` e `d + i`; a implementação
  e o teste dão `double` (`sdk:tests/language/operator/
  number_operator_typing_test.dart:523-578`). Variáveis de tipo passam por
  `resolveToBound` antes.
* **Unários**: `-e`/`~e` → retorno de `unary-`/`~` (`-<literal>` repassa K);
  `!e` → `bool` (`an611:…/prefix_expression_resolver.dart:77-148, 300-311`).
* **`++`/`--`**: prefixo → resultado do operador; **sufixo → tipo de leitura
  do operando** (`an611:…/prefix_expression_resolver.dart:211-247`;
  `postfix_expression_resolver.dart:161-187`).
* **Composta** `a op= b`: `refineBinaryExpressionType(leitura, op, b,
  retorno)`; `=` → tipo do lado direito
  (`an611:…/assignment_expression_resolver.dart:258-293`).
* **Índice**: `[]` → retorno; alvo `Never` → `Never`; dinâmico → `dynamic`
  (`an611:lib/src/generated/resolver.dart:2988-3041`).
* `==`/`!=` → `bool`, busca em NonNull(esquerda), argumento contra parâmetro
  anulável (`nnbd/feature-specification.md:807-824`); `is` → `bool`; `as T` →
  `T`; `&&`/`||` → `bool`.

Oráculo (mem07): `a + a` → `int`; `a + b` → `double`; `a + c` → `num`; `b + a`
→ `double`; `a / a` → `double`; `a ~/ b` → `int`; `a % b` → `double`; `-a`,
`~a` → `int`; `a.remainder(b)` → `double`; `a.clamp(1, 2)` → `int`;
`a.clamp(1.5, 2)` → `num`; `T extends int`: `t + t` → `int`, `t + 1.5` →
`double`, `t - a` → `int`, `a * t` → `int`; `U extends num`: `u * u` → `num`.
(mem08) `i++`, `++i`, `i += 1` → `int`; `d += 1` → `double`; `num n; n +=
1.5` → **`double`**; `l[0]` → `int`; `l[0] += 1`, `l[0]++` → `int`; `m['a']` →
`int?`; `m['a'] ??= 2` → `int`; `ln?[0]` → `int?`. (mem09) `a == o`, `a <
2`, `o is int`, `!b`, `a != 1`, `identical(a, o)` → `bool`; `o as num` →
`num`.

### 8.6 Cascatas, `this`, `super` (R-MEM-10)

Cascata: alvo com o contexto da cascata, seções com `_`, tipo = tipo do alvo
(`an611:lib/src/generated/resolver.dart:2145-2172`;
`static_type_analyzer.dart:80-82`); `?..` mantém `T?`. Oráculo (mem10):
`[1]..add(2)..length` → `List<int>`; `StringBuffer()..write('a')` →
`StringBuffer`; `super.nome()` → `String` (nó `SuperExpression` com tipo da
classe atual `B`); `this` → `B`.

## 9. Null safety: `!`, `??`, `??=`, encurtamento nulo, `Never`/`Null`

### 9.1 NonNull e `e!` (R-NUL-01)

NonNull (texto `lang:accepted/2.12/nnbd/feature-specification.md:1055-1069`;
`an611:…/type_system.dart:1500-1531`): `Null` → `Never`; `X & T` → `X &
NonNull(T)`; `X` → `X & NonNull(limite)` (sem limite: `X & Object`; se não
muda, `X`); `T?` → NonNull(T); o resto sem `?`. `e!`: operando com `K?`,
**continua o encurtamento** (`a?.b!` fica na cadeia), tipo NonNull(T)
(`an611:…/postfix_expression_resolver.dart:189-210`). Oráculo (nul01): `x!` →
`int`; `l!.first` → `int`; `T? t; t!` → `T & Object`; `T t2; t2!` → `T &
Object`; `U extends Object?; u!` → `U & Object`.

### 9.2 `??` e `??=` (R-NUL-02, R-UP-08)

§1.3 e §4.6: `e1` com `K?`; `e2` com K (ou T1 sem contexto); tipo
`UP(NonNull(T1), T2)` com a regra de contexto de *inference-update-3*
(`an611:…/binary_expression_resolver.dart:153-217`; `??=`
`assignment_expression_resolver.dart:292-324`). Oráculo (nul02): `a ?? b`
(`int?`, `double`) → `num`; `a ?? null` → `int?`; `s ?? 'x'` → `String`;
`n ??= 1` (`num?`) → `num`; `a ?? a` → `int?`.

### 9.3 Encurtamento nulo (R-NUL-03)

Texto (`nnbd/feature-specification.md:1407-1546`): `?.` inicia; `.f`, `.m()`,
`(args)`, `[e]`, `!` e atribuições continuam (PASSTHRU); **operadores
terminam** (`e?.f + b` é erro). O tipo da cadeia inteira fica anulável **uma
vez**, no término; **dentro da cadeia os tipos intermediários não são
anuláveis**. [3.6] pilha local `_unfinishedNullShorts` e
`nullShortingTermination` (`an611:lib/src/generated/resolver.dart:259,
1241-1253`; cascatas não ficam anuláveis ali, `:1248`); a extensão da cadeia
por nó está em `an611:lib/src/dart/ast/ast.dart` (`_extendsNullShorting`:
atribuição `:968`, cascata `:2280`, chamada de função `:8720`, índice
`:10390`, invocação de método `:12262`, sufixo `:14324`, prefixo `++/--`
`:14535`, acesso a propriedade `:14689`). [3.14] `NullShortingMixin`
compartilhado (`sdk:pkg/_fe_analyzer_shared/lib/src/type_inference/
null_shorting.dart:36-128`). `?.` em receptor não anulável é aviso, e o tipo
ainda fica anulável.

Oráculo (nul03): `a?.b.c` → `int?` e o `a?.b` **interno** → `B`; `a?.b.m()`
→ `int?`; `a?.b` sozinho → `B?`; `a?.bn?.c` → `int?` (interno `a?.bn` →
`B?`); `a?.b.c.isEven` → `bool?` (internos `B`, `int`); `l?..add(1)` →
`List<int>?`; `l?[0]` → `int?`; `l?.length` → `int?`.

### 9.4 `throw`, `null`, `Never`, `Null`, `late` (R-NUL-04)

`throw e` → `Never` (operando com contexto `Object`); `rethrow` → `Never`;
`null` → `Null`; expressão `Never` torna o fluxo inalcançável
(`an611:lib/src/generated/static_type_analyzer.dart:212-214, 225-227`).
`var x = null` → `dynamic`; `var x = throw …` → `Never`
(`sdk:tests/language/nnbd/never/never_error_test.dart:96-99`). Oráculo
(nul04): `b ? 1 : throw 0` → `int`; `null` → `Null`; `[null]` → `List<Null>`.

## 10. Padrões

Texto: `lang:accepted/3.0/patterns/feature-specification.md` (esquemas
`:1753-1890`, tipo requerido e fluxo do valor `:1892-2154`, `switch`
`:2156-2163`, exaustividade `:2476-2534`) e `exhaustiveness.md`.
Implementação: `fas76:lib/src/type_inference/type_analyzer.dart` (3.6) /
`sdk:pkg/_fe_analyzer_shared/lib/src/type_inference/type_analyzer.dart`
(3.14), com a cola do analyzer em `generated/resolver.dart`.

### 10.1 Esquema do padrão (contexto do inicializador) (R-PAD-02)

Só em declaração e atribuição de padrão. Linhas de 3.14 (`type_analyzer.dart`;
as funções existem com os mesmos nomes em `fas76`):

| padrão | esquema | 3.14 |
|---|---|---|
| e-lógico | DOWN dos lados | 1367-1372 |
| `p!` | `esquema(p)?` | 1716-1723 |
| variável / curinga `T x` | T; sem tipo → `_` | 752-758, 2747-2753 |
| cast | `_` | 593 |
| lista `<T>[…]` | `List<T>`; vazia → `List<_>`; senão DOWN dos esquemas dos elementos (resto `...s` contribui o T de `Iterable<T>`) | 1294-1330 |
| mapa | `Map<K,V>` explícito; senão `Map<_, DOWN(valores)>` | 1623-1655 |
| registro | registro dos esquemas dos campos | 2208-2223 |
| objeto `C(…)` | o tipo escrito, instanciado para os limites provisoriamente | 1832-1834 |
| ou-lógico, `p?`, constante, relacional | só em contexto refutável (recuperação) | |

Declaração: esquema → inicializador com esse contexto (`:2024-2045`);
atribuição `:1846-1860`. Oráculo (pad02): `var (double a, b) = (1, 2)` → o
`1` é `double`, o `2` `int`; `final [double x] = [1]` → `List<double>`; `var
(num c, int d) = (1, 2)` → `(int, int)` (o literal é o mais específico).
(pad07) `var Caixa(:valor) = Caixa(1)` → `valor: dynamic` (esquema
`Caixa<dynamic>` pelo tipo cru instanciado para os limites); `var
Caixa<num>(valor: w) = Caixa(2)` → `Caixa<num>`, `w: num`.

### 10.2 Tipo requerido e fluxo do valor casado M (R-PAD-01, 04, 05, 06)

* **Variável `T x`/`var x`/`final x`**: tipo T, ou
  `variableTypeFromInitializerType(M)` (Null → `dynamic`, rebaixado) sem
  tipo; em contexto irrefutável, `M` não `<: T` é erro salvo `dynamic`
  (`sdk:…:686-745`; [3.6] `fas76:…:501`).
* **Curinga**: não liga; tipado promove.
* **Cast `p as T`**: subpadrão casa `T`; requerido `Object?`.
* **`p?` / `p!`**: o subpadrão casa NonNull(M); `p?` em contexto irrefutável
  é erro.
* **Constante**: valor com contexto M. **Relacional `op c`**: operador buscado
  em M (`TypePropertyResolver`); operando com `A?` para `==`/`!=`, `A` senão;
  retorno não atribuível a `bool` é erro. Em valor anulável, `> 0` é erro
  (`unchecked_use_of_nullable_value` — medido ao escrever pad05).
* **E-lógico**: esquerda com M, direita com o tipo promovido.
  **Ou-lógico**: só refutável; variáveis fundidas.
* **Lista**: E = argumento explícito, senão o de `List<T>` supertipo de M
  (`asInstanceOf(List)`), senão `dynamic`/`Object?`; requerido `List<E>`;
  elementos casam E; resto casa `List<E>` (`sdk:…:1197-1287`; [3.6]
  `fas76:…:798-806`).
* **Mapa**: K, V explícitos ou de M; chaves com contexto K; valores casam V;
  resto é erro (`sdk:…:1496-1616`; [3.6] `fas76:…:1074-1089`).
* **Registro**: requerido = mesma forma com campos `Object?`; campos casam os
  de M se M tem a mesma forma, senão `dynamic`/`Object?`; depois o valor é
  promovido ao tipo de registro demonstrado (`sdk:…:2124-2195`; [3.6]
  `fas76:…:1612`).
* **Objeto `C(f: p)`**: requerido X = C com argumentos de tipo inferidos
  **para baixo a partir de M** (`constrainReturnType(C<T…>, M)` +
  `chooseFinalTypes`, `sdk:pkg/analyzer/lib/src/generated/resolver.dart:
  864-908, 5693-5717`); cada campo é getter ou tear-off buscado por
  `TypePropertyResolver`; X `dynamic`/inválido/`Never` → todos os campos com
  esse tipo. Teste: `sdk:tests/language/patterns/
  object_pattern_inference_test.dart:24-117`.

Oráculo: (pad01) `var (a, b) = (1, 'x')` → `int`, `String`; `final [x, y] =
[1, 2.5]` → `num`, `num`; `var {'k': v} = {'k': 1.5}` → `double`; `var (n:
z)` → `bool`; `var (p, q: r) = (1, q: [1])` → `int`, `List<int>`. (pad04)
`Ponto(x: var px, :var y)` → `int`, `double`; `[var h, ...var t]` (`List<int>`)
→ `int`, `List<int>`; `{'a': var a}` (`Map<String, num>`) → `num`; `List<int>
li` → `List<int>`; `o` `Object` e `[var e]` → `e: Object?`. (pad05) `case var
v? when v > 0` → `int`; `var (a!, b) = (n, 1)` → `int`, `int`; `var [c as int,
d as String] = <Object>[…]` → `int`, `String`; `int j && > 3` → `int`; `int()
|| double()` não promove `o` (`Object`). (pad06) `for (var (i, s) in l)` →
`int`, `String`; `for (var MapEntry(:key, :value) in m.entries)` → `String`,
`int`.

### 10.3 Expressão `switch` e guardas (R-PAD-03)

`sdk:…/type_analyzer.dart:2354-2501` ([3.6] `fas76:…:1779-1894`): escrutínio
com `_`; **sem casos → `Never`**; braços com K; T = UP em dobra; S = fecho maior
de K; regra de *inference-update-3* (§4.6). Guardas com contexto `bool`.
Oráculo (pad03): `switch (o) { int i => i, String s => s.length.toDouble(), _
=> 0 }` → `num`; `switch (fo) { Q() => 1, R() => 'r' }` → `Object`; (up08) com
contexto `B1` → `B1`.

### 10.4 Exaustividade (esboço)

Obrigatória em `switch` **expressão** sempre; em **instrução** só para tipos
*sempre exaustivos*: `bool`, `Null`, enums, `sealed`, `T?`, `FutureOr<T>`,
registros desses, variáveis de tipo e `X & T` com esses limites
(`isAlwaysExhaustive`, `an611:…/type_system.dart:832-844`; `sdk:…:844-888`;
tipos de extensão pelo apagamento). Algoritmo (`exhaustiveness.md:272-885`;
`fas76:lib/src/exhaustiveness/exhaustive.dart`, entrada `:39`): espaço
`Space(raiz, tipo do valor)`; casos **com guarda não cobrem** (`:58`); cada
caso é testado contra os anteriores sem guarda (inalcançável); `_unmatched`
por colunas: sem colunas → coberto se sobra linha, senão **testemunha**;
na primeira coluna, cada espaço: pula subtipos de `Never` (`:137`); tipo
"selado" (se divide) testado inteiro e, se não coberto, dividido
(`getSubtypes`); senão `_filterByType` mantém as linhas cujo tipo é
**supertipo** do valor e expande propriedades (getters, campos de registro,
cabeça/cauda/resto de lista) em colunas novas. Tipos que se dividem: `bool`;
enum (valores); classe `sealed` (subclasses diretas); `T?` → `T | Null`;
`FutureOr<T>` → `T | Future<T>`; `List<T>` por comprimentos `0..n-1` + `n+`
(`sdk:pkg/_fe_analyzer_shared/lib/src/exhaustiveness/types/*.dart`). Fora do
escopo do `crates/types` hoje (é diagnóstico, não tipo), mas o tipo `Never`
de `switch` sem casos e a alcançabilidade depois de `switch` exaustivo
dependem dele.

## 11. Casos de borda dos testes do SDK

`references/dart-sdk/tests/language/` (3.14; a regra é a mesma em 3.6 salvo
indicação):

| # | teste | afirma |
|---|---|---|
| 1 | `inference_update_3/if_null_test.dart:97-99` | `(int? ?? double)` com contexto `Object` → `num` (T <: S, fica T) |
| 2 | `inference_update_3/if_null_test.dart:133-153` | `C1<int>? ?? C2<double>` com contexto `B1<_>` → `B1<Object?>`; com `B1<Object>` → `B1<Object>` |
| 3 | `inference_update_3/if_null_test.dart:167-172` | `Iterable<int>? ?? List<num>` com contexto `Iterable<num>` → `Iterable<num>` (UP seria `Object`) |
| 4 | `inference_update_3/switch_expression_test.dart:117-121, 185-188` | braços `C1<int>`/`C2<double>` com `B1<_>` → `B1<Object?>`; braços `null`/`int` → `int?` |
| 5 | `inference_update_1/horizontal_inference_enabled_test.dart:13-16, 64` | parâmetro de closure inferido de argumento anterior; `f(0, (x) => [x])` → `List<int>` |
| 6 | `inference_using_bounds/restricting_choices_using_bounds_test.dart:20-39` | [3.7+] limites restringem a escolha: `foo1<T extends Object>(FutureOr<Object?>)` → `Object`; `foo2<T extends num>(Null)` → `num` |
| 7 | `inference_update_2/basic_field_promotion_test.dart:46-56` | campo final privado promove (`int`); público não (`int?`) |
| 8 | `inference_update_2/promotion_makes_new_extension_available_via_non_nullability_test.dart:16-51` | depois de `_a != null`, getter, método, `call`, `[]`, `[]=` e `+` de extensão em `A` se aplicam |
| 9 | `inference_update_4/assignment_promotion_in_if_statement_test.dart:22-30` | [não liberado] `(x ??= f()) == null` promove `x` no `else` |
| 10 | `nnbd/static_errors/unchecked_use_of_nullable_test.dart:19-50` | em `int?`: `isEven`, `round()`, `+`, `-`, `++`, `[]`, `+=`, tear-off são erro; `toString()`, `hashCode`, `runtimeType`, `??=`, `x?.isEven` ok; `bool?` em condição é erro |
| 11 | `nnbd/static_errors/equals_parameter_made_nullable_at_invoke_test.dart:21-31` | `Object == null` e `== FutureOr<int?>` válidos |
| 12 | `nnbd/resolution/null_assertion_null_type_test.dart:12-15` | `Null n; f(n!)` ok: NonNull(Null) = `Never` |
| 13 | `nnbd/resolution/question_dot_produces_nullable_type_test.dart:13-26` | `x?.bitLength + 1` é erro (anulável, operador termina a cadeia) |
| 14 | `nnbd/resolution/question_question_lub_test.dart:10-24` | `int? ?? int` → `int`; `int ?? int?` → `int?` + código morto |
| 15 | `nnbd/never/never_error_test.dart:16-99` | todo membro de `Never` é `Never` (inclusive `x == x`); `3 == x` → `bool`; extensões não se aplicam implicitamente a `Never`; `var t = throw "x"` → `Never` |
| 16 | `nnbd/inference/variables_initialized_to_null_test.dart:49-55` | `var local0 = null;` e `var local1 = null as Null;` → `dynamic` |
| 17 | `operator/number_operator_typing_test.dart:55-224` | `int+int` `int`, `int+double` `double`, `int+num` `num`, `int+dynamic` `num`, `int+Never` `num`, `Never+d` `Never`, `clamp` |
| 18 | `operator/number_operator_typing_test.dart:523-578` | `I extends int`: `i+i` `int`; `D extends double`: `d+i` `double` (o texto diria `D`) |
| 19 | `extension_methods/static_extension_resolution_test.dart:167-229` | interface vence extensão; on-type instanciado mais específico vence; `on T` instanciado ao receptor é o mais específico |
| 20 | `extension_methods/static_extension_silly_types_test.dart:28-57` | extensões `on void`, `on dynamic`, `on FutureOr<Object>`, `on Null` casam `null`; `on Function` casa funções |
| 21 | `call/implicit_tearoff_exceptions_test.dart:39-74` | tear-off de `call` aplicado à cascata inteira e ao condicional inteiro (`(b ? c : a)` tem tipo `A`) |
| 22 | `call/method_implicit_tear_off_nullable_test.dart:14-23` | sem tear-off de `C?` |
| 23 | `generic_methods/explicit_instantiated_tearoff_test.dart:17-24` | `staticMethod<int, String>` → `int Function(String, [String?])` |
| 24 | `patterns/schema_test.dart:17-107` | esquema: DOWN de tipos de função (`void Function(num)`), `p!` anulável, `var [int x]` → `List<int>` |
| 25 | `patterns/object_pattern_inference_test.dart:24-117` | argumentos do padrão de objeto do escrutínio (`C<num>`; `C<dynamic>` de `Object`; `D<num>` pelo limite; F-limitado `F1<F1<Object?>>`) |
| 26 | `patterns/empty_switch_expression_test.dart:16-27` | `switch` vazio em `sealed` sem subtipos → `Never`, depois inalcançável |
| 27 | `patterns/exhaustiveness/null_type_test.dart:8-46` | `Null _`/`null` cobre a metade nula de `int?`; duplicado inalcançável; faltando → não exaustivo |
| 28 | `sdk:pkg/analyzer/test/src/dart/resolution/type_inference/function_expression_test.dart:19-1109` | retorno de closures: `() {}` → `Null`; `if (b) return 0;` → `int?`; `num Function() v = () => 0` → `int Function()`; `() sync* { yield 0; return; }` → [3.14] `Iterable<int>` ([3.6] `Iterable<int?>`, clo04) |

## 12. O que muda de 3.6 a 3.14

| área | 3.6 (oráculo) | 3.x+ | onde |
|---|---|---|---|
| *inference-using-bounds* | desligado | 3.7: restrição do limite `Mb <# B`; guarda em R-RES-03 | §2.8 |
| restrições de funções genéricas | sem fecho sobre os Z | com fecho | §2.2 R-RES-18 |
| substituição do limite na solução | `_` para os posteriores | os já fixados | §2.4 |
| elementos nulos `?e` | indisponível | 3.8 | §3.3 |
| *dot shorthands* (`.nome` pelo contexto) | indisponível | 3.10 | — |
| `return;` em gerador | acrescenta `Null` | ignorado | §5.3 |
| contexto de gerador/async pelo tipo imposto | `asInstanceOf` direto | `unionFreeType` | §5.2 |
| DOWN: teste de subtipo | cru | fechos maiores de `_` | §4.3 |
| UP: profundidade de aplicação de mixin nomeada | sem ajuste | −1 | §4.2.4 |
| UP: guarda de profundidade | nenhuma | `Object?` no estouro | §4.2 |
| `switch` expressão: fecho de K | `Object?` | `topType: dynamic` só no CFE | §4.6 |
| `sound-flow-analysis` | ausente | 3.9 (10 regras) | §7.18 |
| suspensão (`await`/`yield` em função local) | não demove | demove | §7.11 |
| campo promovido a `Null` e alcance | não | 3.7 | §6.1 |
| `?.` em `Never?` com nome fora de `Object` | resolução normal | `Never?` | §8.1 |
| extensões estáticas | não | experimental | — |
| nomes internos | `getMember`/`getMember2`, resolvedores unidos, pilha local de encurtamento | `getMember3`/`getMember`, resolvedores divididos, `NullShortingMixin` | §8, §9.3 |

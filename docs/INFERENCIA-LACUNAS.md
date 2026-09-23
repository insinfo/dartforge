# Lacunas do motor de inferência (`crates/types`) contra a especificação

Regras de `docs/INFERENCIA-ESPECIFICACAO.md` que o nosso motor
(`crates/types/src/inferencia/`, `bounds.rs`, `subtyping.rs`, `ops.rs`,
`resolve.rs`) não segue. É a fila do dono do `crates/types` — este documento
não altera código. Cada item traz a regra (R-…), um exemplo mínimo com o
resultado do **oráculo** (analyzer 6.11, gravado em `corpus/inferencia/`) e
onde está o nosso código (linhas lidas em `0ac6357`).

Como confirmar/fechar um item: o teste `crates/types/tests/corpus_inferencia.rs`
despeja cada programa do corpus e o compara com o `.esperado.tsv`; o que ainda
diverge está em `corpus/inferencia/divergencias.txt` (catraca, só encolhe)
(docs/INFERENCIA-ESPECIFICACAO.md §0.3).

## Situação medida (teste do corpus, dono do `crates/types`)

Reconciliado pelo teste do corpus sobre o main depois de `dcedb20`
(programas iguais ao oráculo: 77/95; depois das correções abaixo, 82/95).

| item | situação | evidência |
|---|---|---|
| L01 | aberta | ctx12 (4 divergências) |
| L02 | aberta | lit03 (2) |
| L03 | aberta | gen14 (6 + 1 aviso) |
| L04 | aberta | gen13 (6); nul01 (limite `Object?` escrito não aparece no tipo) |
| L05 | parcial: posição e ordem feitas em `cf59fe9`; falta o getter implícito de campo inferido | ctx11 (3) |
| L06 | resolvida antes de `d5ca257` (Left Null com `X?`) | sem programa |
| L07 | resolvida (UP de tipos de extensão: mesma declaração, `Object?` no conjunto de superinterfaces) | up09 igual |
| L08 | aberta (sem medida) | sem programa |
| L09 | aberta | clo04 (4) |
| L10 | sem medida | — |
| L11 | resolvida em `6487c26` (`Type::Intersection`) | flu06, flu13 iguais |
| L12 | resolvida em `3e4c604` | flu12 igual |
| L13 | resolvida (escrutínio promovido ao tipo casado no ramo que casa, antes da guarda) | flu14 igual |
| L14 | resolvida (escritas do membro inteiro, inclusive nos parâmetros, demovem dentro do literal) | flu07 igual |
| L15 | aberta | flu20 (1) |
| L16 | resolvida em `f05aa61`/`8ca11ec` | flu15 igual |
| L17 | resolvida (variável de condição guarda verdadeiro/falso; versões de escrita por variável) | flu03 igual |
| L18 | sem medida | — |
| L19 | aberta | ext02 (3 + 1 aviso) |
| L20 | aberta | mem01 (1) |
| L21 | aberta | mem11 (3) |
| L22 | resolvida em `f05aa61` | gen01 igual |
| L23 | resolvida em `67203d2` (renomeia os parâmetros, regra 17) | mem11 sem aviso |
| L24–L27 | sem medida | — |
| L28 | aberta | mem03 (1) |
| novas | ext04 resolvida (construtor primário e campo de representação, R-EXT-04); abertas: flu16 (2), flu20 `Null`×`Never?` (1) | ext04 igual |

Situação: **confirmada** = o código lido faz outra coisa que o oráculo grava;
**provável** = o código lido diverge da regra, sem programa no corpus ainda.

## Prioridade (as 10 de maior impacto)

Ordem pelo que o oráculo mede no `new_sali` (docs/FRONTEND-NEW-SALI.md) e
pelo alcance da regra:

1. **L11** interseção `X & S` (sem `Type::Intersection`; ~170 causas medidas
   e bloqueia NonNull de variável de tipo, UP R-UP-g/h e `flatten`).
2. **L22** tear-off genérico como alvo de chamada guarda o tipo instanciado
   (~800 causas medidas).
3. **L16** promoção de `?.` vaza para fora da cadeia (~450 causas medidas).
4. **L12** promoção de campo final privado (3.2).
5. **L05** *override inference* por nome, só com anotações escritas, ordem
   não determinística.
6. **L20** assinatura combinada de membros herdados.
7. **L03** inferência horizontal sem fases por dependência.
8. **L19** override explícito de extensão `E(x).m`.
9. **L14/L15/L13** captura por closure, `finally`, escrutínio de padrão.
10. **L02/L01/L04** spread na desambiguação conjunto/mapa, contexto de `e!`,
    instanciar para os limites.
## Parte 1 — contexto, argumentos de tipo, literais, UP/DOWN, closures

| # | regra | exemplo → oráculo | nosso código | situação |
|---|---|---|---|---|
| L01 | R-CTX-12: o operando de `e!` recebe `K?` | `T? g<T>(); int x = g()!;` → `g<int>()`, `int?` (ctx12) | `inferencia/expr.rs:958-960` infere o operando com `_` → `g<dynamic>` | confirmada |
| L02 | R-LIT-03 passo C: sem argumentos de tipo, sem folhas e sem contexto, os **spreads** decidem conjunto/mapa (spread de Iterable → conjunto) | `var b = {1}; var g = {...b};` → `Set<int>` (lit03) | `inferencia/colecoes.rs:38-61` (spread não decide) e `:139-158` (sem contexto → mapa; `_els` não é usado) → `Map<dynamic, dynamic>` | confirmada |
| L03 | R-GEN-05/12/14: **todo** literal de função é adiado (anotado ou não); fases por dependência (componentes fortemente conexos); solução preliminar recalculada entre fases, não antes da primeira | `V h<T,U,V>(V Function(U) a, U Function(T) b, T c)`; `h((u) => u.isEven, (t) => t * 2, 1)` → `u: int`, `bool` (gen14) | `inferencia/chamadas.rs:77-90` (só literal com parâmetro sem tipo é adiável) e `:157-187` (uma fase só, na ordem do texto, recálculo antes de cada argumento): `a` é inferido antes de `b` fixar `U` → `u: dynamic` | provável (o código não tem as fases; conferir com gen14) |
| L04 | R-GEN-13 (instanciar para os limites, §2.7): algoritmo por componentes fortemente conexos; limite `Object?` **escrito** não vira `dynamic` | `class A<T extends U, U extends num>`: `A a = A()` → `A<num, num>`; `class O<T extends Object?>`: `O()` → `O<Object?>` (gen13) | `inferencia/tipos.rs:250-263` e `resolve.rs:730-743`: outros parâmetros trocados por `dynamic` (→ `A<dynamic, num>`), e `Object?` (implícito ou escrito, indistinguíveis) → `dynamic` | confirmada |
| L05 | R-CTX-11 (*override inference*): assinatura combinada dos supertipos, parâmetro posicional por **posição**, inclusive membros implícitos (getter de campo) e membros também inferidos; ordem determinística | `class A { void m(int a, [String? s]) {} final num campo = 1; var lista = [1.5]; }`, `class B extends A { m(b, [t]) {…} get campo => 2; get lista => []; }` → `b: int`, `t: String?`, `B().campo: num`, `[]: List<double>` (ctx11) | `resolve.rs:776-883` (casa por nome; só anotações escritas no AST do super; `resolve.rs:787, 840` percorrem supertipos de `hierarchy.rs:19`, um `HashMap` — ordem não determinística) | provável |
| L06 | subtipagem (`subtyping.md`, "Left Null"): `Null <: X?` é verdadeiro (a regra "T1 é variável de tipo → falso" é para `X` sem `?`) | `T? g<T>() { T? a = null; return null; }` — 0 diagnósticos no analyzer | `subtyping.rs:450-453`: `matches!(t1, Type::TypeParameter { .. })` casa também `X?` e devolve falso antes do teste de anulável; afeta também `DOWN(Null, X?)` (R-DOWN-h) | confirmada (leitura) |
| L07 | R-UP-w: tipo de extensão é `InterfaceType` no analyzer — mesma declaração → argumento a argumento pela variância | `extension type E<T>(T x) {}`; `b ? E<int>(1) : E<double>(1.5)` → `E<num>`; `b ? E<int>(1) : 1` → `Object?` (up09) | `bounds.rs:459-468`: só `Interface`×`Interface` usa a regra; tipo de extensão vai para `interface_lub` | provável |
| L08 | §4.5 NORM: o analyzer normaliza na combinação de assinaturas de membros, na fusão de superinterfaces e na igualdade em tempo de execução | `FutureOr<Object>` e `Object` são o mesmo tipo onde o analyzer normaliza | `ops.rs:353` (`normalize`) não tem chamador fora de si mesmo; incompleta (sem `T?`, `FutureOr<Null>`, funções, registros, `X extends Never`) | confirmada (leitura) |
| L09 | R-CLO-04 [3.6]: gerador **sem `yield`** tem elemento `dynamic`; `return;` em gerador **acrescenta `Null`** (a regra que o ignora é de 3.14) | `() sync* {}` → `Iterable<dynamic> Function()`; `() sync* { yield 1; return; }` → `Iterable<int?> Function()` (clo04) | `inferencia/funcoes.rs:446-466`: gerador começa em `Never` e ignora `return;` → `Iterable<Never>`, `Iterable<int>` (comportamento de 3.14 no segundo caso) | confirmada; decidir: seguir o oráculo 3.6 (autoridade §0.1) |
| L10 | R-CLO-05: o ajuste ao contexto usa K diretamente (com `_` topo e fundo na subtipagem), não o fecho maior de K | contexto `List<_> Function()` — iguais nos casos medidos | `inferencia/funcoes.rs:470` (`fecho_maior(ctx_ret)`) | provável, impacto baixo |

Não são lacunas (conferido com o oráculo; o levantamento inicial as apontava):

* `f(() => [], <int>[])` com `T f<T>(T Function(), T)` → `List<dynamic>` no
  analyzer também (gen12): o literal sem dependência vai na primeira fase,
  sem recálculo.
* Atribuição a variável promovida usa o tipo **promovido** como contexto
  (`o = [1]` com `o` promovido a `List<num>` → `List<num>`, flu19;
  `an611:lib/src/dart/resolver/flow_analysis_visitor.dart:1136-1145`) — o
  nosso `inferencia/expr.rs:1170-1171` já faz isso.
* Padrão de objeto com classe genérica crua em declaração: `var Caixa(:valor)
  = Caixa(1)` → `valor: dynamic` no analyzer 6.11 (pad07).

## Parte 2 — análise de fluxo e promoção

| # | regra | exemplo → oráculo | nosso código | situação |
|---|---|---|---|---|
| L11 | R-FLU-13 / §7.3: interseção `X & S` — `is`/`as` em variável de tipo com S subtipo do limite, NonNull de variável de tipo, `var` inicializada com `X & S` | `f<T, U extends num?>`: `if (t is int) t` → `T & int`, `t.isEven` → `bool`; `if (u != null) u` → `U & num`; `if (t != null) t` → `T & Object`; `t!` → `T & Object` (flu13, flu06, nul01) | `table.rs:56-98` (`Type` sem interseção); `inferencia/fluxo.rs:163-176` (`None` nos dois ramos: "interseção não representada"); `inferencia/fluxo.rs:199-206` (`T?` de variável de tipo vira `T`) | confirmada; o motor precisa de `Type::Intersection` (também usado por UP R-UP-g/h, `flatten` e `resolveToBound`) |
| L12 | R-FLU-12 / §7.13: promoção de campo final privado (3.2), por `_x`, `this._x` e `outro._x` | `final int? _x`: `if (_x != null) _x` → `int`; `this._x` → `int`; `outro._x` → `int` (flu12) | `inferencia/expr.rs:322-332` (`alvo_de_promocao` só aceita locais) | confirmada |
| L13 | R-FLU-14 / §7.14: `if-case` e `switch` promovem o **escrutínio** variável | `if (o case int i) o` → `int`; `if (x case var v?) x` → `int`; `if (x case != null) x` → `int` (flu14) | `inferencia/padroes.rs:298-310` (sem promoção do escrutínio nem modelo do ramo falso) | confirmada |
| L14 | R-FLU-07 / §7.11 regra 2: dentro de um literal de função, variável escrita **em qualquer lugar** do membro (também depois do literal) perde a promoção | `if (z != null) { () { z; }; z = null; }` → `z: int?` dentro do literal (flu07) | `inferencia/funcoes.rs:284-302` (só as escritas **dentro** do literal contam) | confirmada |
| L15 | R-FLU-20 / §7.12: o estado depois de `try/finally` incorpora as promoções do `finally` (`attachFinally`, `rebase`) | `try { … } finally { o as int; } o` → `int` (flu20) | `inferencia/instrucoes.rs:296-302` ("vale o estado depois do `try`": descarta o `finally`) | confirmada |
| L16 | R-FLU-15 / §7.15: a promoção do alvo de `?.` vale só dentro da cadeia; `a?.v != null` não promove `a` em 3.6 | `y?.isEven; y` → `int?` (flu20); `if (a?.v != null) a` → `A?` (flu15) | `inferencia/expr.rs:583-588` promove o alvo no fluxo corrente sem restaurar no fim da cadeia (docs/FRONTEND-NEW-SALI.md diz "corrigido na árvore de trabalho"; não está em `0ac6357`) | confirmada (leitura) |
| L17 | §7.10: variáveis de condição — a leitura de `var t = x != null && …` restaura a informação de promoção | `var t = x != null && o.isEmpty; if (t) x` → `int` (flu03) | nenhum estado de condição por versão em `inferencia/fluxo.rs` | provável |
| L18 | §7.1 R-FLU-P1: no ramo verdadeiro, `T` só entra em `tested` se promoveu (no falso, sempre) | efeito só em tipos de interesse posteriores | `inferencia/fluxo.rs:155-162` acrescenta `T` a `testados` também quando já era subtipo | provável, impacto baixo |

## Parte 3 — membros, extensões, `call`, tear-offs, null safety, padrões

| # | regra | exemplo → oráculo | nosso código | situação |
|---|---|---|---|---|
| L19 | §8.2: override explícito de extensão `E(x).m` / `E<T>(x).m` | `A([1]).q` → `String`; `X('a').length` → `String` (ext02) | `inferencia/chamadas.rs:395-398` devolve `None` quando o alvo é referência a extensão, e `inferencia/expr.rs:147` resolve `E` como `Type` → "não invocável", `dynamic` | confirmada (leitura) |
| L20 | §8.1: assinatura combinada de membros herdados de várias interfaces (o candidato cujo tipo é subtipo de todos; senão `topMerge`) | `J implements I1 (num get v), I2 (int get v)`: `j.v` → `int`, elemento `GETTER:I2.v` (mem01) | `inferencia/membros.rs:73-84` devolve a primeira declaração na ordem de `supertipos_ordenados` (aqui `I1.v`: `num`) | confirmada (leitura) |
| L21 | §8.3: tear-off implícito de `call` tem o tipo de `call` e entra na inferência assim | `T f<T>(T Function() g)`; `f(C())` (`int call()`) → `T = int`, `int` (mem11) | `inferencia/tipos.rs:219-233` só aceita na verificação de atribuibilidade; o tipo do argumento fica `C` e a restrição `C <# T Function()` não gera `T :> int` → `dynamic` | provável |
| L22 | §8.4: o identificador de função genérica chamada guarda o tipo **genérico** (a instanciação fica na invocação) | `id(1)`: `id` → `T Function<T>(T)` (gen01, e todo `SimpleIdentifier` de chamada genérica no corpus) | `inferencia/chamadas.rs:279-282, 289-292` (e `:376-378`) registram o tipo instanciado no alvo (docs/FRONTEND-NEW-SALI.md: "corrigido na árvore de trabalho"; não está em `0ac6357`) | confirmada (leitura) |
| L23 | subtipagem de funções genéricas: renomear os parâmetros de tipo (limites iguais) antes de comparar | `S Function<S>(S) g = id;` válido (mem11) | `subtyping.rs:272-275` só compara as quantidades; os tipos dos parâmetros são comparados por identidade de `TypeParamId` | provável (aviso espúrio) |
| L24 | §8.1: `super.m` usa `superImplemented` (implementações concretas da cadeia de superclasses e mixins), não interfaces | `class B extends A implements I`: `super.m()` só acha `m` concreto de `A`/mixins | `inferencia/expr.rs:742-749` percorre `supertipos_ordenados` (inclui interfaces) | provável |
| L25 | §8.2: especificidade de extensões — regra da plataforma e desempate pelos on-types instanciados para os limites | `E1<T> on SubTarget<T>` vence `E5<T> on T` (`sdk:tests/language/extension_methods/static_extension_resolution_test.dart:29, 210`) | `inferencia/membros.rs:252-292` compara só os on-types instanciados | provável |
| L26 | §4.6 / §9.2: `??=` também usa a regra de contexto de *inference-update-3* | (sem caso sem erro de atribuição no corpus) | `inferencia/expr.rs:1212-1214` usa `UP` puro | provável, impacto baixo |
| L27 | §10.1: esquema do e-lógico é DOWN dos dois lados | `var (void Function(int) x && void Function(double) y) = …` → contexto `void Function(num)` (`sdk:tests/language/patterns/schema_test.dart:17-19`) | `inferencia/padroes.rs:25-33` toma o primeiro lado conhecido | provável |
| L28 | §8.1: propriedade inexistente em receptor `Never` | `falha().x` → `InvalidType` no 6.11 (mem03) | o nosso resolve membro em `Never` como `Never` (`inferencia/membros.rs`, receptor `Never`) | provável, impacto baixo |

# Lacunas do motor de inferência (`crates/types`) contra a especificação

Regras de `docs/INFERENCIA-ESPECIFICACAO.md` que o nosso motor
(`crates/types/src/inferencia/`, `bounds.rs`, `subtyping.rs`, `ops.rs`,
`resolve.rs`) não segue. É a fila do dono do `crates/types` — este documento
não altera código. Cada item traz a regra (R-…), um exemplo mínimo com o
resultado do **oráculo** (analyzer 6.11, gravado em `corpus/inferencia/`) e
onde está o nosso código (linhas lidas em `0ac6357`).

Como confirmar/fechar um item: rodar o `despejo_tipos` sobre o programa do
corpus indicado e comparar com o `.esperado.tsv` pelo `comparar.py`
(docs/INFERENCIA-ESPECIFICACAO.md §0.3).

Situação: **confirmada** = o código lido faz outra coisa que o oráculo grava;
**provável** = o código lido diverge da regra, sem programa no corpus ainda.

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

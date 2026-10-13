#### 4. Padrões constantes, relacionais e de mapa

Quem relata o quê:

| código | relator | fase |
|---|---|---|
| `CONSTANT_PATTERN_WITH_NON_CONSTANT_EXPRESSION` | **só** `ConstantVerifier.visitConstantPattern` (`:137-140`); a resolução não o emite (busca do nome em `analyzer/lib/src` e `_fe_analyzer_shared/lib`: única ocorrência) | ConstantVerifier |
| `NON_CONSTANT_RELATIONAL_PATTERN_EXPRESSION` | **só** `ConstantVerifier.visitRelationalPattern` (`:384-387`) | ConstantVerifier |
| `CONSTANT_PATTERN_NEVER_MATCHES_VALUE_TYPE` | **só** `ConstantVerifier.visitConstantPattern` (`:148-156`) | ConstantVerifier |
| `matchedValueType` de cada padrão | gravado na resolução: `ResolverVisitor.dispatchPattern` (`resolver.dart:793-811`, `node.matchedValueType = analysisResult.matchedValueType`), vindo de `flow.getMatchedValueType()` no início de `analyzeConstantPattern` (`type_analyzer.dart:418-463`, `:424`) | resolução |
| `INVALID_CONSTANT_PATTERN_*`, `INVALID_CONSTANT_CONST_PREFIX` | parser (família E) | parser |
| `RELATIONAL_PATTERN_OPERAND_TYPE_NOT_ASSIGNABLE`, `REFERENCED_BEFORE_DECLARATION`, `PATTERN_NEVER_MATCHES_VALUE_TYPE` | resolução (`type_analyzer.dart:1691-1740`; `resolver.dart:614-640`) | resolução |

A resolução (`analyzeConstantPattern`) só infere a expressão com o tipo casado como contexto, chama `flow.constantPattern_end` e, sem
`patterns`, compara tipos (`caseExpressionTypeMismatch`); não decide constância.

##### 4.1 `visitConstantPattern` (`:131-162`)

```
e = node.expression.unParenthesized
se e.staticType é InvalidType: return                          // nome não resolvido: só o erro da resolução
v = _evaluateAndReportError(e, CONSTANT_PATTERN_WITH_NON_CONSTANT_EXPRESSION)
se v é valor:
   se feature patterns:
      _constantPatternValues?[node] = v                         // para a exaustividade do switch envolvente
      se v.hasPrimitiveEquality(featureSet):
         ct = v.type                                            // tipo do VALOR
         mt = node.matchedValueType?.extensionTypeErasure
         se mt != null e !_canBeEqual(ct, mt): → R CONSTANT_PATTERN_NEVER_MATCHES_VALUE_TYPE no ConstantPattern [mt, ct]; return
   desce                                                        // só agora: criações/literais const dentro da expressão
```

`_canBeEqual(ct, vt)` (`:517-546`) — devolve `false` só quando prova que nunca é igual:

| `ct` (constante) | `vt` (casado, já sem tipos de extensão) | resultado |
|---|---|---|
| `InterfaceType` `int` | `InterfaceType` `double` (com ou sem `?`: `isDartCoreDouble` não olha o sufixo — não verificado para `double?`) | `true` |
| `InterfaceType` | `InterfaceType` (inclui `Null`, `FutureOr<…>`, enum, `Object`) | `isSubtypeOf(ct, eliminateToGreatest(vt))`, onde o fecho troca parâmetro de tipo covariante por `Object?` e contravariante por `Never` (`least_greatest_closure.dart:98-129`) |
| `InterfaceType` | `TypeParameterType` | `b = promotedBound ?? element.bound`; se `b != null` e `!hasTypeParameterReference(b)`: `_canBeEqual(ct, vt tem '?' ? makeNullable(b) : b)`; senão `true` |
| `InterfaceType` `Null` | `FunctionType` | `isNullable(vt)` |
| outro `InterfaceType` | `FunctionType` | `false` |
| `InterfaceType` | `RecordType`, `dynamic`, `Never`, `void`, `InvalidType` | `true` |
| não `InterfaceType` (função, registro) | qualquer | `true` |

O tipo de um valor `null` é `Null` (`InterfaceType`); o de `#a` é `Symbol`; o de um literal de tipo é `Type`; o de `const Nm('a')` com
`extension type const Nm(String s)` é `String` (o valor é o da representação).

**Qual é o tipo casado.** `flow.getMatchedValueType()` = último tipo promovido da "variável" do valor casado, ou o tipo declarado dela
(`flow_analysis.dart:5737-5744`). É promovido:
- pelos **casos anteriores** do mesmo `switch`: cada alternativa começa no estado "não casou" da anterior
  (`switchStatement_beginAlternative` `:5335-5340`, `switchStatement_endAlternative` `:5376-5384`); um caso com guarda (`when`) não
  promove o que vem depois (o "não casou" junta o estado de antes);
- por `constantPattern_end` → `_handleEqualityCheckPattern` (`:4489-4505`, `:5832-5905`), com `_equalityCheck` (`:5642-5673`):
  constante `null` (`rhsInfo.isNull`) e tipo casado **não** não-anulável → o "não casou" recebe o valor casado promovido a não nulo
  (`_nullCheckPattern`, `:5998-6035`: `tryMarkNonNullable`; se o valor casado é uma variável ou propriedade, ela também é promovida);
  tipo casado `Null` e constante `null` → garantidamente igual (o "não casou" fica inalcançável); constante `null` com tipo casado não
  anulável → nenhuma informação; qualquer outra constante (`true`, `1`, enum) → nenhuma promoção (o fluxo não conhece valores);
- por padrões de tipo anteriores (`bool _`, `int x`, objeto) pelo `promoteForPattern` (família B, §0 T6);
- por `&&`: o lado direito vê o que o esquerdo promoveu (`!= null && null` → `bool`); em `||`, o lado direito parte do "não casou" do
  esquerdo;
- sem `patterns` (biblioteca < 3.0), `constantPattern_end` não promove nada (`:4495-4504`) e o código nem existe.

Posição: o `ConstantPattern` (`beginToken = constKeyword ?? expression.beginToken`, `ast.dart:4059-4062`): inclui `const` e os
parênteses de `const (…)`; **não** inclui os parênteses de um `ParenthesizedPattern` em volta (`(true)` → o `true`). Mensagem:
`{0}` = tipo casado **apagado** (`DartType` → `getDisplayString`, com alias; interseção `T & int`; `FutureOr<int>`; `int Function()`),
`{1}` = tipo do valor.

Exemplos (oráculo vivo 3.6.2) — `e01.dart`, `void f(int x, int y, String s, int? n, double d, num m)`:

| linha: entrada | diagnóstico |
|---|---|
| 2–5: `switch (s) { case 1: … case 'a': … case null: … }` | `constant_pattern_never_matches_value_type off=82 len=1 3:10 \| The matched value type 'String' can never be equal to this constant of type 'int'.`; `off=122 len=4 5:10 \| … 'String' … 'Null'.` |
| 7–11: `switch (n) { case 1: case null: case null: case 'a': }` | 2º `null`: `unreachable_switch_case off=199 len=4 10:5` + `constant_pattern_never_matches_value_type off=204 len=4 10:10 \| … 'int' … 'Null'.`; `'a'`: `off=226 len=3 11:10 \| … 'int' … 'String'.` (o 1º `null` promoveu `int?` → `int`) |
| 13–16: `switch (d) { case 1: case 1.5: case 'a': }` | só `off=306 len=3 16:10 \| … 'double' … 'String'.` (`int` × `double` aceito) |
| 18–20: `switch (x) { case 1.5: case 2: }` | nenhum (`double` não tem igualdade primitiva) |
| 22–25: `switch (m) { case 1: case 1.5: case true: }` | `off=445 len=4 25:10 \| … 'num' … 'bool'.` (+ `non_exhaustive_switch_statement off=383 len=6 22:3`) |

`e03.dart` (`enum E {a, b}`, `enum F {c}`, `extension type Id(int i) {}`, `extension type const Nm(String s) {}`, `class A`, `class B extends A`,
`class Eq` com `==`/`hashCode`; parâmetros `E e, F g, Id id, FutureOr<int> fo, FutureOr<int>? fon, A a, B b, Object o, dynamic dy,
int Function() fn, void Function()? fq, (int, String) r, Null nu, Never nv`):

| linha: `if (… case …)` | diagnóstico (`constant_pattern_never_matches_value_type` salvo indicação) |
|---|---|
| 10: `e case F.c` | `off=440 len=3 10:14 \| … 'E' … 'F'.` |
| 11: `e case E.a` | nenhum |
| 12: `g case null` | `off=482 len=4 12:14 \| … 'F' … 'Null'.` |
| 13: `id case 1` | nenhum |
| 14: `id case 'a'` | `off=525 len=3 14:15 \| … 'int' … 'String'.` (tipo de extensão apagado) |
| 15: `id case const Nm('a')` | `off=547 len=13 15:15 \| … 'int' … 'String'.` |
| 16–19: `fo case 1` / `fo case 'a'` / `fo case null` / `fon case null` | 17: `off=599 len=3 \| … 'FutureOr<int>' … 'String'.`; 18: `off=621 len=4 \| … 'FutureOr<int>' … 'Null'.`; 16 e 19: nenhum |
| 20–22: `a case const B()` / `b case const A()` / `b case const Eq()` | 21: `off=694 len=9 21:14 \| … 'B' … 'A'.`; 20 e 22 (sem igualdade primitiva): nenhum |
| 23–24: `o case 1`, `dy case 1` | nenhum |
| 25–28: `fn case 1` / `fn case null` / `fq case null` / `fn case f` | 25: `off=789 len=1 \| … 'int Function()' … 'int'.`; 26: `off=809 len=4 \| … 'int Function()' … 'Null'.`; 27, 28: nenhum |
| 29, 31: `r case 1`, `r case const (1, 2)` | nenhum (registro: sem suporte) |
| 32–34: `nu case 1` / `nu case null` / `nv case 1` | 32: `off=950 len=1 \| … 'Null' … 'int'.`; 33: nenhum; 34: só `dead_code` |
| 35–39: `o case int` / `e case #a` / `e case const [1]` / `e case const {1: 2}` / `e case 1.5` | 36: `off=1033 len=2 \| … 'E' … 'Symbol'.`; 37: `off=1053 len=9 \| … 'List<int>'.`; 38: `off=1080 len=12 \| … 'Map<int, int>'.`; 35, 39: nenhum |

`e04.dart`, `void f<T, U extends num, V extends List<V>, W extends U>(T t, U u, V v, T? tq, U? uq, W w, Object? x)`:

| linha: entrada | diagnóstico |
|---|---|
| 2: `t case 1` (sem limite) | nenhum |
| 3–5: `u case 1` / `u case 'a'` / `u case null` | 4: `off=155 len=3 4:14 \| … 'U' … 'String'.`; 5: `off=176 len=4 5:14 \| … 'U' … 'Null'.` |
| 6–7: `uq case null` / `uq case 'a'` | 7: `off=222 len=3 7:15 \| … 'U?' … 'String'.` |
| 8–10: `v case 1` (limite menciona parâmetro) / `w case 'a'` (limite `U`) / `tq case null` | nenhum |
| 11–14: `if (t is int) { t case true; t case (true); t case 1 }` | `off=326 len=4 12:16 \| … 'T & int' … 'bool'.`; `off=351 len=4 13:17` (o `true` dentro do padrão parentizado); `1`: nenhum |

`e05.dart`, `void f(bool? b, int x)`:

| linha: entrada | diagnóstico |
|---|---|
| 2–7: `switch (b) { case null: case true: case null: case false: }` | `unreachable_switch_case off=88 len=4 5:5`; `constant_pattern_never_matches_value_type off=93 len=4 5:10 \| … 'bool' … 'Null'.` |
| 8–13: `switch (b) { null => 0, Null _ => 1, null => 2, _ => 3 }` | `pattern_never_matches_value_type off=175 len=4 10:5`; `unreachable_switch_case off=182 len=2 10:12`; `constant_pattern_never_matches_value_type off=192 len=4 11:5 \| … 'bool' … 'Null'.`; `unreachable_switch_case off=197 len=2 11:10` |
| 14–19: `switch (b) { case true: case bool _: case null: case 1: }` | `case 1`: só `dead_code` (18:5 e 18:13) — o tipo casado já é `Never` |
| 20–25: `switch (x) { case int _ when x > 0: case 'a': case == 'a': case > 'a': }` | `off=384 len=3 22:10 \| … 'int' … 'String'.`; `== 'a'`: nenhum; `> 'a'`: `relational_pattern_operand_type_not_assignable off=431 len=3 24:12` |
| 26: `if (b case true \|\| null) {}` | nenhum |
| 27: `if (b case != null && null) {}` | `off=501 len=4 27:25 \| … 'bool' … 'Null'.` |
| 28: `if (b case == null) {} else if (b case null) {}` | `off=551 len=4 28:42 \| … 'bool' … 'Null'.` (a variável `b` foi promovida no `else`) |

##### 4.2 `visitRelationalPattern` (`:381-388`)

Desce (o operando pode conter criação/literal const) e avalia `node.operand` com padrão
`NON_CONSTANT_RELATIONAL_PATTERN_EXPRESSION`. Não há teste de `InvalidType` aqui (operando não resolvido → o `InvalidConstant` do
identificador não resolvido vem com `avoidReporting` — D.R4-1). O operando **não** é contexto constante (§1.2): `> [1]` avalia uma lista
não const (genérico no nó). Não há "nunca casa" para relacionais.

##### 4.3 `visitMapPattern` (`:311-358`)

Visita `typeArguments`; para cada elemento: `element.accept(this)` (o padrão do valor; a chave também é visitada como filha) e, se é
`MapPatternEntry`, `_evaluateAndReportError(key, NON_CONSTANT_MAP_PATTERN_KEY)`; com valor: grava em `_mapPatternKeyValues` e procura
em `uniqueKeys` (igualdade: `isIdentical2` verdadeiro, ou os dois com igualdade primitiva e `==`). Duplicatas → `EQUAL_KEYS_IN_MAP_PATTERN`
na chave repetida, contexto na primeira (`diagnostic_factory.dart:247-264`).

##### 4.4 `switch`: exaustividade e o legado

`_validateSwitchExhaustiveness` (`:883-1007`) usa os `_constantPatternValues`/`_mapPatternKeyValues` colhidos nas visitas acima (o
`PatternConverter` lê o valor de cada `ConstantPattern`; padrão constante sem valor marca `hasInvalidType`); relata
`UNREACHABLE_SWITCH_CASE` (token `case` ou a seta), `NON_EXHAUSTIVE_SWITCH_*` (token `switch`), `UNREACHABLE_SWITCH_DEFAULT`. Não decide
`CONSTANT_PATTERN_NEVER_MATCHES_VALUE_TYPE` (isso é só o `_canBeEqual` de §4.1); os dois coexistem (e01 linha 10). O algoritmo de
espaços é da família B.

Sem `patterns` (`_validateSwitchStatement_nullSafety`, `:1009-1046`): `SwitchCase.expression` (ou a expressão de um `ConstantPattern`
de `SwitchPatternCase`) → `_evaluateAndReportError(expr, NON_CONSTANT_CASE_EXPRESSION)`; com valor sem igualdade primitiva →
`CASE_EXPRESSION_TYPE_IMPLEMENTS_EQUALS` na expressão, argumento = tipo do valor.

Exemplo — `e07.dart` (`// @dart=2.19`):

```dart
// @dart=2.19
class Eq { const Eq(); bool operator ==(Object o) => true; }
void f(int x, int y, String s) {
  switch (x) {
    case 1: break;
    case y: break;
    case 'a': break;
  }
  switch (s) {
    case const Eq(): break;
  }
}
```
```
non_constant_case_expression off=151 len=1 6:10 | Case expressions must be constant.
case_expression_type_is_not_switch_expression_subtype off=170 len=3 7:10 | The switch case expression type 'String' must be a subtype of the switch expression type 'int'.
case_expression_type_implements_equals off=210 len=10 10:10 | The switch case expression type 'Eq' can't override the '==' operator.
case_expression_type_is_not_switch_expression_subtype off=210 len=10 10:10 | The switch case expression type 'Eq' must be a subtype of the switch expression type 'String'.
```


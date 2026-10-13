#### 2. "Potencialmente constante" (`potentially_constant.dart`)

`getNotPotentiallyConstants(node, featureSet)` (`:20-29`) devolve a **lista de nós** que não são potencialmente constantes; é puramente
sintático + elemento resolvido (não avalia nada). `_Collector.collect` (`:60-212`), na ordem dos testes do fonte:

| nó | regra | linha |
|---|---|---|
| `BooleanLiteral`, `DoubleLiteral`, `IntegerLiteral`, `NullLiteral`, `SimpleStringLiteral`, `SymbolLiteral` | aceito | `:61-68` |
| `AdjacentStrings` | cada string | `:70-75` |
| `StringInterpolation` | cada `InterpolationExpression.expression` | `:77-84` |
| `Identifier` (simples ou `a.b`) | `_identifier` (abaixo) | `:86-88` |
| `InstanceCreationExpression` | não `isConst` → o nó inteiro; const → aceito **sem olhar argumentos nem tipo** | `:90-95` |
| `TypedLiteral` | `_typedLiteral` (abaixo) | `:97-99` |
| `ParenthesizedExpression` | a expressão | `:101-104` |
| `RecordLiteral` | cada campo (com ou sem `const`) | `:106-108`, `:323-327` |
| `MethodInvocation` | só `identical(a, b)` de `dart:core` com exatamente 2 argumentos (colhe os dois); qualquer outra → o nó inteiro | `:110-112`, `:278-290` |
| `NamedExpression` | a expressão | `:114-116` |
| `BinaryExpression` | os dois operandos (qualquer operador, inclusive `??`) | `:118-122` |
| `PrefixExpression` | `!`, `-`, `~` → o operando; outro (`++`, `--`) → o nó | `:124-134` |
| `ConditionalExpression` | condição, então, senão | `:136-141` |
| `PropertyAccess` | `_propertyAccess` (abaixo) | `:143-145` |
| `AsExpression`, `IsExpression` | com `non_nullable`: tipo que não é `isPotentiallyConstantTypeExpression` → o **nó do tipo**; sem: `isConstantTypeExpression`; depois a expressão | `:147-173` |
| `MapLiteralEntry` | chave, valor | `:175-179` |
| `SpreadElement` | a expressão | `:181-184` |
| `IfElement` | condição, então, senão | `:186-193` |
| `ConstructorReference` | `_typeArgumentList(constructorName.type.typeArguments)` | `:195-198` |
| `FunctionReference` | `_typeArgumentList(typeArguments)`; depois a função | `:200-204` |
| `TypeLiteral` | `_typeArgumentList(type.typeArguments)` | `:206-209` |
| qualquer outro (`PostfixExpression`, `IndexExpression`, `ThisExpression`, `FunctionExpression`, `SwitchExpression`, `ThrowExpression`, `AwaitExpression`, `AssignmentExpression`, `CascadeExpression`, `ForElement`…) | o nó inteiro | `:211` |

`_identifier` (`:214-276`), com `element = node.staticElement`:

```
se PrefixedIdentifier:
   se isDeferred: → nó; return
   se identifier.name == 'length': collect(prefix); return            // QUALQUER x.length, sem olhar o tipo de x
   se element é MethodElement estático: se prefix não é nome de tipo constante (_isConstantTypeName: InterfaceElement/TypeAliasElement
                                        não diferido) → nó; return
se element é ParameterElement:
   se enclosing é ConstructorElement const (ou marcado em temporaryConstConstructorElements, só o lint prefer_const_constructors,
      linter.dart:464) E o nó está dentro de um ConstructorInitializer: aceito
   senão → nó
se element é VariableElement (local): !isConst → nó; return
se element é PropertyAccessorElement getter: variable2 == null → aceito; variável não const → nó; return
      // variável de topo/campo estático lidos por nome; getter declarado tem variável sintética não const → nó
se _isConstantTypeName(node): aceito            // classe, enum, mixin, extension type, typedef
se FunctionElement: aceito                       // função de topo ou local
se MethodElement estático: aceito
se TypeParameterElement e feature constructor_tearoffs: aceito
senão → nó                                       // não resolvido, método de instância, setter, prefixo…
```

`_propertyAccess` (`:292-321`; `a.b.c`, `(e).p`): `propertyName == 'length'` → collect(target); alvo `PrefixedIdentifier` não diferido
cujo `propertyName` é getter de variável: sem variável → aceito; variável não const → o **`propertyName`**; const → aceito; diferido → nó;
todo o resto → o nó inteiro.

`_typedLiteral` (`:340-385`): literal não `isConst` → o nó inteiro. Lista com 1 argumento de tipo: se não é *potencialmente* constante
→ o nó do tipo. Conjunto/mapa com 1 argumento: idem; com 2 argumentos (mapa): chave e valor têm de ser tipos **constantes**
(`isConstantTypeExpression`; parâmetro de tipo não vale) → o nó do tipo. Depois, cada elemento (`collect`).

`_ConstantTypeChecker.check` (`:399-465`), com `potentially` ligado ou não:

| tipo | potencialmente constante | constante |
|---|---|---|
| `NamedType` de parâmetro de tipo | sim (`:403-407`) | não |
| `NamedType` de classe/enum/mixin/extension type/typedef não diferido | sim se todos os argumentos passam (`:445-452`) | idem |
| `NamedType` com tipo `dynamic`, `Never`, `void` | sim (`:453-456`) | sim |
| outro `NamedType` (não resolvido, diferido) | não | não |
| `GenericFunctionType` | retorno (se escrito), limites dos parâmetros de tipo e tipos dos `SimpleFormalParameter` passam (`:416-443`); o uso de um parâmetro de tipo **do próprio** tipo de função é `NamedType` de parâmetro de tipo: só passa no modo potencial | idem |
| `RecordTypeAnnotation` | todos os campos passam (`:460-465`) | idem |
| tipo ausente (`null`) | não (`:400-402`) | não |

Usos (só três no 3.6.2; busca por `getNotPotentiallyConstants`):

| uso | o que é passado | código relatado, em cada nó devolvido |
|---|---|---|
| `ConstantVerifier._validateConstructorInitializers` (`:790-807`) | condição e mensagem de `AssertInitializer`; expressão de `ConstructorFieldInitializer`; **cada argumento** de `this(...)`/`super(...)` | `INVALID_CONSTANT` (`:749-762`) |
| `_ConstLiteralVerifier._reportNotPotentialConstants` (`:1166-1203`) | o ramo **não tomado** de um `if` em literal const (ou os dois, se a condição é desconhecida) | lista: `NON_CONSTANT_LIST_ELEMENT`; conjunto: `NON_CONSTANT_SET_ELEMENT`; mapa: sobe do nó até a 1ª `MapLiteralEntry` — é a chave → `NON_CONSTANT_MAP_KEY`, senão `NON_CONSTANT_MAP_VALUE`; sem entrada acima → `NON_CONSTANT_MAP_ELEMENT` |
| motor: `ConstantVisitor._reportNotPotentialConstants` (`evaluation.dart:1991-2001`), no operando direito não avaliado de `&&`/`\|\|`/`??` (`:672-688`) e no ramo não tomado de `?:` (`:776-782`) | só o **primeiro** nó vira `InvalidConstant(INVALID_CONSTANT)` | o padrão do chamador (D.R4-1) |

Valor padrão de parâmetro **não** passa por aqui (é avaliado: `_validateDefaultValues`), nem `assert` de corpo. Os parâmetros do
construtor só são aceitos dentro de inicializadores; por isso a visita normal que vem depois (`super.visitConstructorDeclaration`) pega
o que o coletor aceitou mas que precisa de valor: `const [p]` num inicializador é potencialmente constante (literal const, `p`
parâmetro), e o `visitListLiteral` avalia `p` fora do ambiente do construtor → `non_constant_list_element` no `p` (b08, linha 8).

Exemplos (oráculo vivo 3.6.2) — `b08.dart`:

```dart
class A {
  final int x;
  const A(int p) : x = p + f(), assert(p is int), assert(g, 'm${p}');
  const A.b(int p) : x = (p as dynamic).foo;
  const A.c(List<int> p) : x = p.length + p.first;
  const A.d(int p) : this(h);
  const A.e(int p) : x = [p].length;
  const A.g(int p) : x = const [p].length;
}
int f() => 0;
bool g = true;
var h = 1;
```
```
invalid_constant off=52 len=3 3:28 | Invalid constant value.                                   // f()
unnecessary_type_check off=64 len=8 3:40 | Unnecessary type check; the result is always 'true'.
invalid_constant off=82 len=1 3:58 | Invalid constant value.                                   // g (variável de topo não const)
invalid_constant off=120 len=18 4:26 | Invalid constant value.                                 // (p as dynamic).foo: PropertyAccess inteiro
invalid_constant off=182 len=7 5:43 | Invalid constant value.                                  // p.first (p.length é aceito)
invalid_constant off=217 len=1 6:27 | Invalid constant value.                                  // h, argumento de this(...)
invalid_constant off=246 len=3 7:26 | Invalid constant value.                                  // [p]: literal não const
non_constant_list_element off=290 len=1 8:33 | The values in a const list literal must be constants.
```

`f04.dart` (classe `A` com `final Object x; static int s = 0; static const c = 0; static int sm() => 0;`, `import 'dart:math' deferred as m;`,
`const x2 = 0;` no topo; uma linha por construtor):

| linha: inicializador | diagnósticos |
|---|---|
| 8: `x = s` | `invalid_constant off=170 len=1 8:26` (o `s`) |
| 9–11: `x = c`, `x = sm`, `x = A.sm` | nenhum |
| 12: `x = identical(p, s)` | `invalid_constant off=299 len=1 12:39` (só o `s`) |
| 13: `x = p > 0 ? -p : ~p` | nenhum |
| 14: `x = p++` | `invalid_constant off=370 len=3 14:26` (o `PostfixExpression`) |
| 15: `x = m.pi` (prefixo diferido) | `invalid_constant off=400 len=4 15:26` (`m.pi`) |
| 16: `x = (p, a: s)` | `invalid_constant off=438 len=1 16:33` (o `s`) |
| 17: `x = 'a' 'b${s}'` | `invalid_constant off=475 len=1 17:34` (o `s`) |
| 18: `x = #sym` | nenhum |
| 20: `x = p.toString()` | `invalid_constant off=570 len=12 20:26` |
| 21: `x = this` | `invalid_constant off=609 len=4 21:26` + `invalid_reference_to_this off=609 len=4` |
| 22: `A.o(String p) : x = p.length` | nenhum |
| 23: `A.p(String p) : x = p.isEmpty` | `invalid_constant off=681 len=9 23:29` (`p.isEmpty` inteiro) |
| 24: `x = A.s` | `invalid_constant off=717 len=3 24:26` (`A.s` inteiro: `PrefixedIdentifier` de getter de variável não const) |
| 25: `x = p ?? s` | `invalid_constant off=752 len=1 25:31` (o `s`) + `dead_null_aware_expression off=752 len=1` |
| 26: `x = switch (p) { _ => 0 }` | `invalid_constant off=780 len=21 26:26` |
| 27: `x = [p][0]` | `invalid_constant off=828 len=6 27:26` (o `IndexExpression`) |
| 28: `x = p == 0 ? null : throw p` | `invalid_constant off=877 len=7 28:42` (`throw p`) + `const_constructor_throws_exception off=877 len=7` + `field_initializer_not_assignable off=861 len=23` |
| 29: `x = x2` | nenhum |
| 30: `x = await p` | `invalid_constant off=940 len=7 30:26` + `await_in_wrong_context off=940 len=5` |
| 31: `void mm(int q) { const A.a(q); }` | `const_with_non_constant_argument off=978 len=1 31:30 \| Arguments of a constant creation must be constant expressions.` |

`f03.dart` (tipos em inicializadores de `class A<T>` com `final Object x;` e `X idf<X>(X x) => x;`):

| linha: inicializador | diagnósticos |
|---|---|
| 3: `x = const <T>[]` | `invalid_type_argument_in_const_literal off=58 len=1 3:28` (lista: `T` é potencialmente constante → sem `invalid_constant`) |
| 4: `x = const <T, int>{}` | `invalid_constant off=91 len=1 4:28` + `invalid_type_argument_in_const_literal off=91 len=1 4:28` (mapa exige tipo **constante**) |
| 5: `x = const <T>{}` | `invalid_type_argument_in_const_literal off=129 len=1 5:28` |
| 6–10: `x = o is T`, `x = o as List<T>`, `x = T`, `x = List<T>`, `x = A<T>.a` | nenhum |
| 11: `x = const A<T>.a()` | `const_with_type_parameters off=321 len=1 11:29` (do `visitInstanceCreationExpression`, não do coletor) |
| 12: `x = <T>[]` | `invalid_constant off=349 len=5 12:21` (literal não const, nó inteiro) |
| 13: `x = idf<T>` | nenhum |

#### 3. Literais const (`_ConstLiteralVerifier`, `constant_verifier.dart:1064-1389`)

Montagem (`visitListLiteral` `:294-308`, `visitSetOrMapLiteral` `:391-434`), só com `node.isConst`, **depois** de descer nos filhos:

| literal | verificador | código padrão | dados |
|---|---|---|---|
| lista | `listElementType = (staticType as InterfaceType).typeArguments[0]` | `NON_CONSTANT_LIST_ELEMENT` | — |
| conjunto (`isSet`) | `_SetVerifierConfig(elementType)` | `NON_CONSTANT_SET_ELEMENT` | `uniqueValues: Map<DartObject, Expression>`, `duplicateElements` |
| mapa (`isMap`) | `_MapVerifierConfig(keyType, valueType)` | `NON_CONSTANT_MAP_ELEMENT` | `uniqueKeys`, `duplicateKeys` |
| `{}` ambíguo (nem `isSet` nem `isMap`) | nenhum | — | só `ambiguous_set_or_map_literal_*` da resolução (f01, linha 18) |

`verify(elemento)` (`:1079-1163`), devolve `bool` (válido):

```
Expression:
   v = _evaluateAndReportError(el, padrão); se inválido: false
   lista:    !runtimeTypeMatch(v, listElementType) → R LIST_ELEMENT_TYPE_NOT_ASSIGNABLE no elemento [v.type, tipo]; false
   conjunto: !runtimeTypeMatch(v, elementType)     → R SET_ELEMENT_TYPE_NOT_ASSIGNABLE [v.type, tipo]; false
             !v.hasPrimitiveEquality(featureSet)   → R CONST_SET_ELEMENT_NOT_PRIMITIVE_EQUALITY [v.type]; false
             já em uniqueValues → duplicateElements[el] = primeiro; senão uniqueValues[v] = el
   mapa:     true (expressão solta em mapa: o erro EXPRESSION_IN_MAP vem do motor)
ForElement:  → R CONST_EVAL_FOR_ELEMENT no elemento inteiro; false
IfElement:
   c = _evaluateAndReportError(condição, padrão); inválido: false;  !c.isBool: false (já relatado pelo motor)
   b = c.toBoolValue()
   b == null (bool desconhecido, p. ex. fromEnvironment): os DOIS ramos por _reportNotPotentialConstants (§2)
   b == true:  verify(então);  senão-ramo por _reportNotPotentialConstants
   b == false: então-ramo por _reportNotPotentialConstants;  verify(senão)
MapLiteralEntry: _validateMapLiteralEntry (sem mapConfig: false)
   k = _evaluateAndReportError(chave, NON_CONSTANT_MAP_KEY);  val = _evaluateAndReportError(valor, NON_CONSTANT_MAP_VALUE)   // os dois sempre
   k válido: !runtimeTypeMatch(k, keyType)   → R MAP_KEY_TYPE_NOT_ASSIGNABLE na chave [k.type, keyType]
             !k.hasPrimitiveEquality         → R CONST_MAP_KEY_NOT_PRIMITIVE_EQUALITY na chave [k.type]      // não interrompe
             já em uniqueKeys → duplicateKeys[chave] = primeira; senão uniqueKeys[k] = chave
   val válido: !runtimeTypeMatch(val, valueType) → R MAP_VALUE_TYPE_NOT_ASSIGNABLE no valor [val.type, valueType]
   true
SpreadElement:
   v = _evaluateAndReportError(expressão, padrão); inválido: false
   lista/conjunto: nem lista nem conjunto: (v nulo e `...?`) → true; senão → R CONST_SPREAD_EXPECTED_LIST_OR_SET na expressão; false
       conjunto e v é LISTA com algum item sem igualdade primitiva → R CONST_SET_ELEMENT_NOT_PRIMITIVE_EQUALITY no SpreadElement
            inteiro (com `...`), argumento = tipo da lista; false
       conjunto: cada item entra em uniqueValues/duplicateElements com a EXPRESSÃO do spread como nó
   mapa: (v nulo e `...?`) → true; v é mapa: cada chave em uniqueKeys/duplicateKeys com a expressão do spread; senão → R CONST_SPREAD_EXPECTED_MAP
```

Depois de todos os elementos, as duplicatas saem por `DiagnosticFactory`: `EQUAL_ELEMENTS_IN_CONST_SET` / `EQUAL_KEYS_IN_CONST_MAP` no
nó repetido, com mensagem de contexto no primeiro (`:406-409`, `:428-431`). `runtimeTypeMatch` (`evaluation.dart:3207-3214`) =
`isSubtypeOf(obj.type, type.extensionTypeErasure)`. `hasPrimitiveEquality` (`value.dart`): `BoolState`, `IntState`, `StringState`,
`NullState`, `SymbolState`, `TypeState`, `FunctionState`, `ListState`, `SetState`, `MapState` → sim (`:79`, `:2135`, `:3022`, `:2731`,
`:3083`, `:3140`, `:1389`, `:2571`, `:2943`, `:2660`); `RecordState` → todos os campos (`:2831-2835`); `GenericState` (instância de
classe) → `==` concreto vem de `Object` e, com `patterns`, `hashCode` também (`:1496-1523`); `DoubleState` → **não** (padrão de
`InstanceState`, `:1740`). As tabelas de únicos usam `==`/`hashCode` do `DartObjectImpl`.

Interações: elemento que é ele mesmo um literal const (lista dentro de spread) é verificado duas vezes — como filho
(`super.visitListLiteral`) e pela avaliação do spread — com o mesmo código e posição quando o literal interno é do mesmo tipo de
coleção; `not_iterable_spread` (resolução) soma-se a `const_spread_expected_list_or_set`.

`INVALID_TYPE_ARGUMENT_IN_CONST_LIST/MAP/SET` não é daqui (§1.5; ver o código em §5).

Exemplo (oráculo vivo 3.6.2) — `f01.dart`, cabeçalho `class Eq { const Eq(); bool operator ==(Object o) => true; int get hashCode => 0; }`,
`class H { const H(); int get hashCode => 0; }`, `int n = 0; const bool t = true; const u = bool.fromEnvironment('x');`:

| linha: entrada | diagnósticos |
|---|---|
| 6: `var a = const [1, n, 'a'];` | `non_constant_list_element off=217 len=1 6:19 \| The values in a const list literal must be constants.` |
| 7: `var b = const <int>[1, 'a', 1.5];` | `list_element_type_not_assignable off=249 len=3 7:24 \| The element type 'String' can't be assigned to the list type 'int'.`; idem `off=254 len=3 7:29` com `'double'` |
| 8: `var c = const {1, n, 1, 2, 2};` | `non_constant_set_element off=278 len=1 8:19`; `equal_elements_in_const_set off=281 len=1 8:22` (ctx `off=275 len=1 \| The first element with this value.`); `equal_elements_in_const_set off=287 len=1 8:28` (ctx `off=284`) |
| 9: `var d = const {Eq(), H(), 1.5, 2.0};` | `const_set_element_not_primitive_equality off=306 len=4 9:16 \| An element in a constant set can't override the '==' operator, or 'hashCode', but the type 'Eq' does.`; `off=312 len=3` (`'H'`); `off=317 len=3` e `off=322 len=3` (`'double'`) |
| 10: `var e = const {1: n, n: 2, 1: 3};` | `non_constant_map_value off=346 len=1 10:19 \| The values in a const map literal must be constant.`; `non_constant_map_key off=349 len=1 10:22 \| The keys in a const map literal must be constant.`; `equal_keys_in_const_map off=355 len=1 10:28` (ctx `off=343 len=1 \| The first key with this value.`) |
| 11: `var f = const {Eq(): 1, H(): 2, 1.5: 3};` | `const_map_key_not_primitive_equality off=377 len=4 11:16 \| The type of a key in a constant map can't override the '==' operator, or 'hashCode', but the class 'Eq' does.`; `off=386 len=3` (`'H'`); `off=394 len=3` (`'double'`) |
| 12: `var g = const <int, String>{'a': 1};` | `map_key_type_not_assignable off=431 len=3 12:29`; `map_value_type_not_assignable off=436 len=1 12:34` |
| 13: `var h = const [if (t) n else n, if (!t) n else 1];` | `non_constant_list_element` em `off=462` (13:23, ramo tomado, avaliado), `off=469` (13:30, ramo não tomado, coletor), `off=480` (13:41, ramo não tomado) — todos `len=1` |
| 14: `var i = const [if (u) n else 1, if (n > 0) 1];` | `non_constant_list_element off=513 len=1 14:23` (condição desconhecida: coletor nos dois ramos); `off=527 len=1 14:37` (o `n` da condição) |
| 15: `var j = const [for (var k = 0; k < 1; k++) k, for (var k in [1]) k];` | `const_eval_for_element off=553 len=29 15:16 \| Constant expressions don't support 'for' elements.`; `off=584 len=20 15:47` |
| 16: `var k = const [...[1, n], ...{2}, ...1, ...?null, ...n];` | `non_constant_list_element off=629 len=1 16:23`; `const_spread_expected_list_or_set off=644 len=1 16:38 \| A list or a set is expected in this spread.` + `not_iterable_spread off=644 len=1`; `non_constant_list_element off=660 len=1 16:54` + `not_iterable_spread off=660 len=1` |
| 17: `var l = const {...[1, 2], 1, ...{3}, ...[Eq()]};` | `equal_elements_in_const_set off=690 len=1 17:27` (ctx `off=682 len=6`: a expressão do spread); `const_set_element_not_primitive_equality off=701 len=9 17:38 \| … but the type 'List<Eq>' does.` (o `SpreadElement` inteiro) |
| 19: `var o = const {if (u) 1: n else n: 2};` | `non_constant_map_value off=789 len=1 19:26`; `non_constant_map_key off=796 len=1 19:33` |
| 20: `var p = const {if (t) 1 else 2, 1};` | `equal_elements_in_const_set off=835 len=1 20:33` (ctx `off=825 len=1`): só o ramo tomado entra nos únicos |


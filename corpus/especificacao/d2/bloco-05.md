##### `invalid_type_argument_in_const_literal` (perda 16: FN 16, FP 0, msg 0, pos 0)
Nome publicado de `INVALID_TYPE_ARGUMENT_IN_CONST_LIST`, `…_MAP`, `…_SET` (`messages.yaml:9133`, `:9187`, `:9195`).
- **Emissão:** `TypeArgumentsVerifier.checkListLiteral` / `checkMapLiteral` / `checkSetLiteral`
  (`analyzer/lib/src/error/type_arguments_verifier.dart:159-173`, `:175-189`, `:208-222`), chamados de `ErrorVerifier.visitListLiteral`
  (`error_verifier.dart:1154`) e `visitSetOrMapLiteral` (`:1393` se `isMap`, `:1397` se `isSet`); recursão em
  `_checkTypeArgumentConst` (`:501-544`). Fase ErrorVerifier.
- **Condição exata:** literal com argumentos de tipo **escritos** e `node.isConst` (const escrito ou contexto constante); para cada
  argumento:
  ```
  NamedType(type, typeArguments): type é TypeParameterType → R no NamedType, arg = name2.lexeme
                                  senão recursão nos argumentos
  GenericFunctionType: para cada SimpleFormalParameter com tipo: tipo é TypeParameterType → R no tipo, arg = o DartType; senão recursão
                       depois o retorno, igual
                       (limites dos parâmetros de tipo do tipo de função NÃO são olhados; os parâmetros de tipo do PRÓPRIO tipo de
                        função contam como parâmetro de tipo — não há conjunto de permitidos)
  RecordTypeAnnotation: cada campo: tipo é TypeParameterType → R no tipo do campo, arg = o DartType; senão recursão
  ```
  Um `{}` que a resolução não decide (nem `isMap` nem `isSet`) não é checado.
- **Posição:** o nó do tipo (`T` len 1; `T?` len 2).
- **Mensagem:** "Constant {list|map|set} literals can't use a type parameter in a type argument, such as '{0}'." `{0}`: no ramo
  `NamedType`, o lexema (sem `?`: `Map<int, T?>` → `'T'`); nos ramos de função/registro, o `DartType` (com `?`: `({T? x})` → `'T?'`).
- **Supressões e ordem:** nenhuma; independe da avaliação. Em inicializador de construtor const soma-se ao `invalid_constant` do
  coletor só no **mapa** (tipo de chave/valor precisa ser constante; f03 linha 4); lista e conjunto só dão este código. Em valor padrão
  (`p = const <T>[]`) **não** sai `non_constant_default_value` (o literal avalia). Literal sem `const` em valor padrão
  (`Object r = <T>[]`) não é `isConst`: só `non_constant_default_value` no literal (f02 linha 14).
- **No DartForge:** não existe (busca por `INVALID_TYPE_ARGUMENT_IN_CONST` em `crates/`: nada fora da tabela de códigos). As 16 FN são a
  regra ausente (`invalid_type_argument_in_const_{list,map,set}/*` 11, `const/constant_type_variable_error_test.dart` 3,
  `list/literal6_test.dart:10:20`, `map/literal14_test.dart:9:28`). Mudança: em `verificador.rs::expr`, ramos `List` (`:613-629`) e
  `SetOrMap` (`:630-659`), quando `c` é verdadeiro e há argumentos de tipo escritos; lista/mapa/conjunto pelo tipo estático (como o
  `TipoLiteral` de `:641-647`); precisa da mesma função "tipo escrito é parâmetro de tipo" de `const_with_type_parameters`, incluindo os
  parâmetros do próprio tipo de função.
- **Exemplos (oráculo vivo 3.6.2):** `f02.dart` (dentro de `class C<T> { m() { … } }`):

| linha: entrada | diagnóstico |
|---|---|
| 3: `var a = const <T Function(int)>[];` | `invalid_type_argument_in_const_literal off=40 len=1 3:20 \| Constant list literals can't use a type parameter in a type argument, such as 'T'.` |
| 4: `var b = const <int Function(T)>[];` | `off=92 len=1 4:33` (`'T'`) |
| 5: `var c = const <(T, int)>[];` | `off=119 len=1 5:21` (`'T'`) |
| 6: `var d = const <({T? x})>{};` | `off=152 len=2 6:22 \| Constant set literals … such as 'T?'.` |
| 7: `var e = const <List<T>, Map<int, T?>>{};` | `off=187 len=1 7:25 \| Constant map literals … 'T'.`; `off=200 len=2 7:38` (`'T'`, nó `T?`) |
| 8: `var f = const <void Function<X>(X)>[];` | `off=244 len=1 8:37` (`'X'`: parâmetro do próprio tipo de função) |
| 9: `var g = const <X Function<X extends T>()>[];` | `off=270 len=1 9:20` (`'X'`, o retorno); o limite `T` não é relatado |
| 10: `const h = [<T>[]];` | `off=316 len=1 10:17` (lista interna const por contexto) |
| 11: `var i = const [<T>{}];` | `off=343 len=1 11:21 \| Constant set literals …` |
| 14: `void n({List<T> p = const <T>[], Map<T, int> q = const <T, int>{}, Object r = <T>[]}) {}` | `off=423 len=1 14:30` (lista), `off=452 len=1 14:59` (mapa), `non_constant_default_value off=474 len=5 14:81` |

  `a05.dart` (em `class C<T> { const C(); m() {…} }`): `var b = const <T>[];` → `off=79 len=1 5:20` (lista);
  `var c = const <T, int>{};` → `off=104 len=1 6:20` (mapa); `var d = const <T>{};` → `off=134 len=1 7:20` (conjunto);
  `var f = const <List<T?>>[];` → `off=196 len=2 9:25 | … such as 'T'.`; na mesma classe, `var a = const C<T>();` →
  `const_with_type_parameters off=54 len=1 4:21` e `var e = const C<List<T>>();` → `const_with_type_parameters off=165 len=1 8:26`.

##### `non_constant_relational_pattern_expression` (perda 16: FN 4, FP 0, msg 0, pos 12)
- **Emissão:** `ConstantVerifier.visitRelationalPattern` (`constant_verifier.dart:381-388`), via `_evaluateAndReportError`. Único emissor.
- **Condição exata:** a avaliação de `node.operand` dá `InvalidConstant` com código fora da LISTA (§1.4); os da LISTA saem com o
  próprio nome.
- **Posição:** a do erro do motor: o subnó que falhou (`>= k + y` → o `y`; `> [1]` → o literal; `== b` → o `b`).
- **Mensagem:** "The relational pattern expression must be a constant." Sem argumentos.
- **Supressões e ordem:** sem guarda de `InvalidType` (sai junto com `undefined_identifier` e com `referenced_before_declaration`).
  O operando não é contexto constante.
- **No DartForge:** `crates/types/src/constantes/verificador.rs:553-556` (`PatternKind::Relational` → `avaliar_e_relatar`), fiel.
  - FN (4): `patterns/pattern_variable_constant_scope_test.dart:37:13, 73:8, 107:17, 140:19` — `== b && var b`: o analyzer resolve o
    `b` para a variável do **próprio** padrão (declarada depois; `referenced_before_declaration`), que não é const → genérico no `b`
    (e06 linha 3). Por que o nosso lado não relata (resolução do `b` para a constante externa, ou sem resolução): não verificado.
    As linhas vizinhas `var a && == a` (31, 67…) não estão nas FN (já acertamos).
  - pos (12): `dot_shorthands/equality/equality_error_test.dart` (8) e `equality_ctor_error_test.dart` (4) — oráculo 3.13, atalhos de
    ponto; no primeiro o deslocamento é de 1 coluna (o oficial relata no identificador depois do ponto; nós, a partir do ponto —
    `crates/types/src/constantes/avaliador.rs:541-546`); no segundo o pareamento junta relatos de linhas diferentes (306 × 312): são
    uma FN e um FP. Não conferido no oráculo vivo (3.6.2 não tem a sintaxe).
- **Exemplos (oráculo vivo 3.6.2):** `e02.dart`:

```dart
void f(int x, int y) {
  const k = 1;
  final fi = 2;
  switch (x) {
    case x + 1: break;
    case y: break;
    case k: break;
    case fi: break;
    case k + y: break;
    case > y: break;
    case == fi: break;
    case < k: break;
    case >= k + y: break;
    case const (1 + y): break;
  }
  if (x case y) {}
  if (x case != y when y > 0) {}
}
```
```
constant_pattern_with_non_constant_expression off=78 len=1 5:10 | The expression of a constant pattern must be a valid constant.
invalid_constant_pattern_binary off=80 len=1 5:12 | The binary operator + is not supported as a constant pattern.
constant_pattern_with_non_constant_expression off=101 len=1 6:10 | The expression of a constant pattern must be a valid constant.
constant_pattern_with_non_constant_expression off=139 len=2 8:10 | The expression of a constant pattern must be a valid constant.
invalid_constant_pattern_binary off=161 len=1 9:12 | The binary operator + is not supported as a constant pattern.
constant_pattern_with_non_constant_expression off=163 len=1 9:14 | The expression of a constant pattern must be a valid constant.
non_constant_relational_pattern_expression off=184 len=1 10:12 | The relational pattern expression must be a constant.
non_constant_relational_pattern_expression off=206 len=2 11:13 | The relational pattern expression must be a constant.
non_constant_relational_pattern_expression off=254 len=1 13:17 | The relational pattern expression must be a constant.
constant_pattern_with_non_constant_expression off=284 len=1 14:21 | The expression of a constant pattern must be a valid constant.
constant_pattern_with_non_constant_expression off=312 len=1 16:14 | The expression of a constant pattern must be a valid constant.
non_constant_relational_pattern_expression off=334 len=1 17:17 | The relational pattern expression must be a constant.
```
  `e06.dart` (`void f(int x, Map<String, int> m, List<int> l) { const b = 1; …`):

| linha: entrada | diagnósticos |
|---|---|
| 3: `if (x case == b && var b) {}` | `non_constant_relational_pattern_expression off=80 len=1 3:17` + `referenced_before_declaration off=80 len=1` (ctx `off=89 len=1 \| The declaration of 'b' is here.`) |
| 4: `if (x case var c && == c) {}` | `non_constant_relational_pattern_expression off=120 len=1 4:26` + `referenced_before_declaration off=120 len=1` (ctx `off=112`) |
| 5: `if (m case {b: 1}) {}` | nenhum |
| 6: `if (m case {'a': 1, 'a': 2}) {}` | `equal_keys_in_map_pattern off=172 len=3 6:23` (ctx `off=164 len=3 \| The first key with this value.`) |
| 7: `if (m case {x: 1}) {}` | `non_constant_map_pattern_key off=198 len=1 7:15 \| Key expressions in map patterns must be constants.` |
| 8: `if (l case [1, 'a', null]) {}` | `constant_pattern_never_matches_value_type off=225 len=3 8:18` (`'int'` × `'String'`), `off=230 len=4 8:23` (`'int'` × `'Null'`) |
| 9: `if (m case {'a': 'b'}) {}` | `constant_pattern_never_matches_value_type off=259 len=3 9:20` |

  `i01.dart` (`void f(double? dq, int x, List<int> l)`, `class A { const A(int i); }`, `int n = 0;`):

| linha: entrada | diagnósticos |
|---|---|
| 2–3: `dq case 1` / `dq case 'a'` | 3: `constant_pattern_never_matches_value_type off=75 len=3 3:15 \| … 'double?' … 'String'.`; 2: nenhum |
| 4: `x case > [1]` | `non_constant_relational_pattern_expression off=98 len=3 4:16` + `relational_pattern_operand_type_not_assignable off=98 len=3` |
| 5: `x case == indef` | `non_constant_relational_pattern_expression off=122 len=5 5:17` + `undefined_identifier off=122 len=5` |
| 6: `x case indef` | só `undefined_identifier off=145 len=5 6:14` |
| 7: `x case > const [1]` | só `relational_pattern_operand_type_not_assignable off=170 len=9 7:16` |
| 8: `l case [const A(n)]` | `constant_pattern_with_non_constant_expression off=206 len=1 8:23` (no `n`; não `const_with_non_constant_argument`) |
| 9: `x case const A(n)` | `constant_pattern_with_non_constant_expression off=235 len=1 9:22` |

##### `const_constructor_with_mixin_with_field` (perda 13: FN 13, FP 0, msg 0, pos 0)
Nome publicado de `CONST_CONSTRUCTOR_WITH_MIXIN_WITH_FIELD` e `…_FIELDS` (`messages.yaml:2595`, `:2612`).
- **Emissão:** `ErrorVerifier._checkForConstConstructorWithNonConstSuper` (`error_verifier.dart:2807-2857`, parte dos mixins), de
  `visitConstructorDeclaration:597`. Fase ErrorVerifier.
- **Condição exata:**
  ```
  se _enclosingClass == null ou o construtor não é const: return false
  se factory: return false
  campos = para cada mixin de enclosingClass.mixins (ordem da cláusula with), mixin.element.fields (ordem de declaração) com
           !isStatic && !isSynthetic && !(isAbstract && isFinal)
  1 campo  → R CONST_CONSTRUCTOR_WITH_MIXIN_WITH_FIELD  ["'Mixin.campo'"];            return true
  >1       → R CONST_CONSTRUCTOR_WITH_MIXIN_WITH_FIELDS ["'M1.a', 'M2.b', 'M2.c'"];   return true
  ```
  Contam: `final a = 0`, `int b = 0`, `late final int c`, `abstract int e` (não final). Não contam: `abstract final int d`, campo
  estático, getter declarado. Só os mixins **diretos** da classe (os da superclasse aparecem como super não const).
- **Posição:** `constructor.returnType` — só o nome da classe, também em construtor nomeado (`X2` em `const X2.nome()`, len 2).
- **Mensagem:** "This constructor can't be declared 'const' because a mixin adds the instance field: {0}." / "… because the mixins add
  the instance fields: {0}." `{0}` já vem com aspas simples por nome (`'M1.a'`), separados por `, `.
- **Supressões e ordem:** ao relatar devolve `true`: **não** saem `const_constructor_with_non_const_super` nem
  `const_constructor_with_non_final_field` para o mesmo construtor (b06, classe `B`). Uma vez por construtor gerador const.
- **No DartForge:** não existe. FN: `const_constructor_with_mixin_with_field/*` (12, quatro delas `…__pri_*` de construtor primário,
  oráculo 3.13, posição no nome da classe do cabeçalho) e `const/constructor_mixin2_test.dart:14:9`. Mudança:
  `verificador.rs::construtor` (`:261-303`; `k.const_ && !k.factory`): classe por `classe_de` (`:333-343`), mixins por
  `ClassElement::mixin_classes`, campos por `ClassElement::fields`; `abstract` do campo não está em `VariableElement`
  (`crates/elements/src/model.rs:432-444`) — ler de `ast::VariableList` ou acrescentar o bit. Span = `k.class_name`.
- **Exemplos (oráculo vivo 3.6.2):** `b02.dart`:

```dart
mixin M1 {
  final int a = 0;
}
mixin M2 {
  int b = 0;
  late final int c;
}
mixin M3 {
  abstract final int d;
  static int s = 0;
  int get g => 0;
}
mixin M4 {
  abstract int e;
}
class S {
  const S();
}
class X1 extends S with M1 {
  const X1();
}
class X2 extends S with M1, M2 {
  const X2.nome();
}
class X3 extends S with M3 {
  const X3();
}
class X4 extends S with M4 {
  const X4();
  const factory X4.f() = X4;
}
```
```
const_constructor_with_mixin_with_field off=246 len=2 20:9 | This constructor can't be declared 'const' because a mixin adds the instance field: 'M1.a'.
const_constructor_with_mixin_with_field off=295 len=2 23:9 | This constructor can't be declared 'const' because the mixins add the instance fields: 'M1.a', 'M2.b', 'M2.c'.
non_abstract_class_inherits_abstract_member off=314 len=2 25:7 | Missing concrete implementation of 'getter M3.d'.
non_abstract_class_inherits_abstract_member off=359 len=2 28:7 | Missing concrete implementations of 'getter M4.e' and 'setter M4.e'.
const_constructor_with_mixin_with_field off=390 len=2 29:9 | This constructor can't be declared 'const' because a mixin adds the instance field: 'M4.e'.
```

##### `constant_pattern_with_non_constant_expression` (perda 12: FN 10, FP 0, msg 0, pos 2)
- **Emissão:** `ConstantVerifier.visitConstantPattern` (`constant_verifier.dart:137-140`). Único emissor (§4).
- **Condição exata:** expressão (sem parênteses) com tipo estático ≠ `InvalidType`, e a avaliação dá `InvalidConstant` com código fora
  da LISTA. Não depende de `patterns` estar ligado.
- **Posição:** a do erro do motor:
  | expressão | nó relatado | origem |
  |---|---|---|
  | identificador de local/parâmetro/`final`/`this` | o identificador | `_getConstantValue` / `visitNode` |
  | `x + 1`, `k + y` | o operando não constante | binária avalia os operandos primeiro |
  | `++v`, `--v`, `-v`, `!v` (prefixo) | o **operando** `v` | `visitPrefixExpression` avalia o operando antes de olhar o operador (`evaluation.dart:1158-1161`); só com operando constante é que `++` dá genérico no nó (`:1168-1171`) |
  | `v--`, `v++` (sufixo), `switch (…) {…}`, `await e`, `assert(false)` (chamada de nome indefinido), função literal | o nó inteiro | `visitNode` (`evaluation.dart:1091`) |
  | `const A(n)` | o argumento `n` | `CONST_WITH_NON_CONSTANT_ARGUMENT` substituído pelo padrão |
- **Mensagem:** "The expression of a constant pattern must be a valid constant." Sem argumentos.
- **Supressões e ordem:** `InvalidType` → nada (`case indef`, `case super()` → só o erro da resolução). Os erros de sintaxe do parser
  no mesmo padrão (`invalid_constant_pattern_binary` no operador, `…_unary`, `…_negation`, `…_generic`, `invalid_constant_const_prefix`)
  **não** suprimem: a AST guarda a expressão e ela é avaliada (`!false` avalia: só o erro do parser).
- **No DartForge:** `verificador.rs:530-540` (`PatternKind::Constant` → `avaliar_e_relatar`); guarda de inválido por
  `tipos_invalidos` (`:532`).
  - FN `constant_pattern_with_non_constant_expression/…54b22230.dart:3:14` e `…76def280.dart:4:14` (`case a` com `a` local `var` /
    topo `final`): o parser guarda `case nome` como `PatternKind::Variable` sem `var`/`final`/tipo
    (`crates/frontend/src/parser/patterns.rs:11`); a inferência não o resolve (`padroes.rs:134-138`, retorna) e o verificador só
    consulta constantes de topo para a exaustividade (`verificador.rs:543-552`). Mudança: tratar esse caso como padrão constante de
    identificador — resolver o nome no escopo (local, parâmetro, membro, topo) e avaliar/relatar no nome.
  - FN `…cf905e82.dart:3:16`, `…f0766f4e.dart:3:16`, `patterns/invalid_const_pattern_binary_test.dart:430:12, 438:12`,
    `invalid_const_pattern_test.dart:199:14, 355:20` (`++a`, `--a`, `const ++variable`): `avaliador.rs:941-943` devolve genérico no
    nó **antes** de avaliar o operando; o oficial relata no operando. O nosso relato no nó inteiro (355:18) é o que o placar pareia
    como "posição" com 178:12. Mudança: avaliar o operando primeiro; genérico no nó só se ele for constante.
  - FN `invalid_const_pattern_test.dart:305:26` (`const void fun() {}` → a função literal `() {}` depois do nome descartado, len 5) e
    `316:18` (`const assert(false)` → a chamada inteira, len 13); "posição" `160:12` (`assert(false)`, len 13) × nosso `44:12`
    (`case super()`: FP nosso — o oficial só dá `invocation_of_non_function_expression`, j02) e `178:12` (`await 0`, len 7). Dependem
    da recuperação do parser (família E) e da guarda de `InvalidType`; o que o nosso parser produz nesses casos: não verificado.
- **Exemplos (oráculo vivo 3.6.2):** `e02.dart` acima (linhas 5–9, 14, 16); `g01.dart` (`void f(o) async { var variable = 0; … }`,
  cada `case` no próprio `switch (o)`):

| linha: entrada | diagnósticos |
|---|---|
| 4: `case ++variable:` | `constant_pattern_with_non_constant_expression off=64 len=8 4:12` (o operando) |
| 7: `case const ++variable:` | `off=110 len=8 7:18` |
| 10: `case assert(false):` | `off=148 len=13 10:10` + `undefined_identifier off=148 len=6` |
| 13: `case switch (o) { _ => true }:` | `off=191 len=24 13:10` |
| 16: `case await 0:` | `off=245 len=7 16:10` |
| 19: `case const void fun() {}:` | `named_function_expression off=293 len=3 19:21` + `constant_pattern_with_non_constant_expression off=296 len=5 19:24` |
| 22: `case const assert(false):` | `off=337 len=13 22:16` + `undefined_identifier off=337 len=6` |
| 25: `case -variable:` | `off=381 len=8 25:11` + `invalid_constant_pattern_negation off=381 len=8` |
| 26–27: `case !false:` / `case const !false:` | só `invalid_constant_pattern_unary` (`off=400 len=1`, `off=423 len=1`) |
| 28: `case variable--:` | `off=440 len=10 28:10` (o nó inteiro) |

  `j02.dart` (em método de classe): `case super():` → só `invocation_of_non_function_expression off=52 len=5 4:12`; `case this:` →
  `constant_pattern_with_non_constant_expression off=95 len=4 7:12`; `case o:` (parâmetro) → `off=135 len=1 10:12`.

##### `const_constructor_with_non_const_super` (perda 11: FN 11, FP 0, msg 0, pos 0)
- **Emissão:** `ErrorVerifier._checkForConstConstructorWithNonConstSuper` (`error_verifier.dart:2859-2890`), depois do teste de mixins.
- **Condição exata:**
  ```
  (passou pelo teste de mixins sem relatar; construtor const, não factory)
  se enclosingClass é EnumElement: return false
  se element.redirectedConstructor != null: return false                 // `: this.x()` — é REDIRECT_TO_NON_CONST_CONSTRUCTOR
  s = element.superConstructor; se s == null ou s.isConst: return false
  nó = primeiro SuperConstructorInvocation dos inicializadores ?? constructor.returnType
  R no nó, [element.enclosingElement3.displayName]; return true
  ```
  `superConstructor` (`summary2/super_constructor_resolver.dart:29-63`): com `super(...)`/`super.nome(...)` escrito, o construtor de
  mesmo nome do supertipo; sem `super` nem `this`, o sem nome do supertipo. O construtor padrão **implícito** de uma superclasse sem
  construtores não é const (b07, `class C extends Sem`); o encaminhado de aplicação de mixin segue a regra de `const_with_non_const`.
  Superclasse não resolvida → `null` → nada.
- **Posição:** o `SuperConstructorInvocation` inteiro (`super()` len 7; `super.n()` len 9), mesmo depois de outros inicializadores;
  sem ele, `returnType` (só o nome da classe, também em `const D.nome()`).
- **Mensagem:** "A constant constructor can't call a non-constant super constructor of '{0}'." No 3.6.2, `{0}` =
  `element.enclosingElement3.displayName` = a classe **do construtor declarado** (a subclasse). No main/3.14
  (`E:\references\dart-sdk`, `error_verifier.dart:4749`, de lá) o argumento é `invokedSuper.enclosingElement.displayName` (a
  superclasse) — §0 T2.
- **Supressões e ordem:** suprimido por `const_constructor_with_mixin_with_field`; ao relatar, suprime
  `const_constructor_with_non_final_field` (b06, classe `A`). Classe sem `extends` → `Object`, const.
- **No DartForge:** não existe. FN: `const_constructor_with_non_const_super/*` (8; `…081b577b` e `…107840ce` em 2:7 são construtor
  primário 3.13), `const/constructor_super_test.dart:18:31, 27:9`, `super/non_const_test.dart:12:9`. Mudança:
  `verificador.rs::construtor`, depois do teste de mixins: superclasse por `ClassElement::supertype_class`, construtor alvo por nome em
  `constructors` (`FunctionKind::SyntheticConstructor` padrão → não const; de aplicação de mixin → regra do encaminhado), `const_` da
  declaração original (§0 T3); span do `ast::Initializer::Super` ou `k.class_name`; argumento conforme a versão do oráculo (T2).
- **Exemplos (oráculo vivo 3.6.2):** `b01.dart`:

```dart
class A {
  A();
  A.n();
  const A.c();
}
class B extends A {
  const B();
}
class C extends A {
  const C() : super();
  const C.n() : super.n();
  const C.c() : super.c();
  const C.a(int x) : assert(x > 0), super.n();
}
class D extends A {
  const D.nome();
}
```
```
const_constructor_with_non_const_super off=71 len=1 7:9 | A constant constructor can't call a non-constant super constructor of 'B'.
const_constructor_with_non_const_super off=112 len=7 10:15 | A constant constructor can't call a non-constant super constructor of 'C'.
const_constructor_with_non_const_super off=137 len=9 11:17 | A constant constructor can't call a non-constant super constructor of 'C'.
const_constructor_with_non_const_super off=211 len=9 13:37 | A constant constructor can't call a non-constant super constructor of 'C'.
const_constructor_with_non_const_super off=252 len=1 16:9 | A constant constructor can't call a non-constant super constructor of 'D'.
```
  `b06.dart` (interações; `class S { S(); }`, `mixin M { int m = 0; }`):

| linha: entrada | diagnóstico |
|---|---|
| 4–7: `class A extends S { int x = 0; const A(); }` | só `const_constructor_with_non_const_super off=60 len=1 6:9` (`'A'`); o campo não final não é relatado |
| 11–14: `class B extends S with M { int x = 0; const B(); }` | só `const_constructor_with_mixin_with_field off=140 len=1 13:9` (`'M.m'`) |
| 16: `const C() : this.r();` (em `class C extends S`) | nenhum |
| 17: `const C.r();` | `const_constructor_with_non_const_super off=199 len=1 17:9` (`'C'`) |
| 19: `const C.q() : this.nc();` | `redirect_to_non_const_constructor off=237 len=2 19:22` |

  `b07.dart`: `class A = S with M1; class B extends A { const B(); }` (S sem const) → `off=80 len=1 7:9` (`'B'`);
  `class Sem {} class C extends Sem { const C(); }` → `off=130 len=1 11:9` (`'C'`); `class D extends Indef { const D(); }` → só
  `extends_non_class`; `extension type const ET(int i) { const ET.n() : i = 0; }` → nenhum.

##### `non_const_generative_enum_constructor` (perda 11: FN 11, FP 0, msg 0, pos 0)
- **Emissão:** `ErrorVerifier._checkForNonConstGenerativeEnumConstructor` (`error_verifier.dart:4710-4719`), primeira checagem de
  `visitConstructorDeclaration` (`:594`).
- **Condição exata:** `_enclosingClass is EnumElement && node.constKeyword == null && node.factoryKeyword == null`.
- **Posição:** `atConstructorDeclaration` (`analyzer/lib/error/listener.dart:72-97`): com nome, de `returnType.offset` até o fim do
  nome (`E.nome`, len 6); sem nome, o `returnType` (`E`).
- **Mensagem:** "Generative enum constructors must be 'const'." Sem argumentos.
- **Supressões e ordem:** nenhuma. Vale para o que o parser **recupera** como construtor: `static int E1() => 0;` e `void E3() {}`
  num enum de mesmo nome viram construtores (`static_constructor`, `constructor_with_return_type`) e recebem este código no nome
  (g02); `augment E.named();` no 3.6.2 (sem o experimento) idem (h04).
- **No DartForge:** não existe (`crates/analise/src/enums.rs` só tem `sem_constantes`). FN: `non_const_generative_enum_constructor/*`
  (5), `enum/enhanced_enums_error_test.dart:350:14, 371:7, 493:3` (os dois primeiros são método com nome do enum recuperado como
  construtor), `augmentation_modifier_missing/…a93fc07a.dart:9:11`, `constructor_body/…b4bc584e.dart:8:11` e `…c6f5a1c0.dart:5:11`
  (3.13, não conferidos aqui). Mudança: `verificador.rs::construtor` ou `crates/analise/src/membros.rs` (onde já sai
  `NON_FINAL_FIELD_IN_ENUM`, `:930`): enum, `!k.const_ && !k.factory` → span `k.class_name.start .. k.name.end` (ou `k.class_name`);
  depende de o nosso parser também tratar o método homônimo como construtor (não verificado).
- **Exemplos (oráculo vivo 3.6.2):** `b04.dart`:

```dart
enum E {
  v, w.nome();
  E();
  E.nome();
  factory E.f() => v;
}
enum F {
  a;
  int x = 0;
  const F();
}
```
```
non_const_generative_enum_constructor off=26 len=1 3:3 | Generative enum constructors must be 'const'.
non_const_generative_enum_constructor off=33 len=6 4:3 | Generative enum constructors must be 'const'.
non_final_field_in_enum off=87 len=1 9:7 | Enums can only declare final fields.
```
  `g02.dart`: `enum E1 { v; static int E1() => 0; }` → `static_constructor off=17 len=6 3:3`, `constructor_with_return_type off=24 len=3 3:10`,
  `non_const_generative_enum_constructor off=28 len=2 3:14`, `return_in_generative_constructor off=33 len=5`, `return_of_invalid_type off=36 len=1`;
  `enum E2 { v; E2(); E2.named() : this(); }` → `off=58 len=2 7:3` e `off=66 len=8 8:3` (+ `unused_element off=69 len=5`);
  `enum E3 { v; void E3() {} }` → `constructor_with_return_type off=106 len=4 12:3` + `non_const_generative_enum_constructor off=111 len=2 12:8`.

##### `const_constructor_with_non_final_field` (perda 9: FN 9, FP 0, msg 0, pos 0)
- **Emissão:** `ErrorVerifier._checkForConstConstructorWithNonFinalField` (`error_verifier.dart:2896-2914`), de
  `visitConstructorDeclaration:597-599`, **só se** `_checkForConstConstructorWithNonConstSuper` devolveu `false`.
- **Condição exata:** construtor const e gerador (`_enclosingExecutable.isConstConstructor && isGenerativeConstructor`, `:69-99`);
  `classElement is ClassElement` (não enum, mixin, extension type) e `hasNonFinalField` (`element.dart:329-358`): busca em largura na
  classe, nos mixins e nas superclasses por um campo `!isFinal && !isConst && !isStatic && !isSynthetic`.
  Contam: `var x`, `int x = 0`, `abstract int y`, `external int e`, campo não final herdado ou de mixin. Não contam: `final`,
  `late final`, `static var`, par getter/setter declarado (campo sintético).
- **Posição:** `atConstructorDeclaration`: `A` (len 1) ou `A.nome` (do nome da classe ao fim do nome do construtor).
- **Mensagem:** "Can't define a const constructor for a class with non-final fields." Sem argumentos.
- **Supressões e ordem:** suprimido por mixin-com-campo e por super não const (os dois testes vêm antes e devolvem `true`): o campo
  herdado de superclasse cujo construtor não é const sai como super não const (b03 `B`); só aparece "herdado" quando a superclasse
  tem construtor const (g03 `D`). Uma vez por construtor; factory const não. Soma-se a
  `const_constructor_with_field_initialized_by_non_const` (token `const`) quando o campo tem inicializador (b05).
- **No DartForge:** não existe. FN: `const_constructor_with_non_final_field/*` (7; `…prim_*` em 1:7 são construtor primário 3.13),
  `const/constructor_syntax_test.dart:36:9`, `augmentation_modifier_extra/…e8d84b00.dart:3:17` (no 3.6.2 `augment const A();` é
  recuperado como campo `augment` + construtor `const A()`: o campo recuperado é o "não final"; h04). Mudança:
  `verificador.rs::construtor`, depois dos dois testes anteriores; `ClassKind::Class`/`MixinApplication`; busca por `fields` da classe,
  `mixin_classes` e `supertype_class` (`final_`, `const_`, `static_` em `VariableElement`; sintético = sem declaração de campo).
- **Exemplos (oráculo vivo 3.6.2):** `b03.dart`:

```dart
class A {
  int x = 0;
  const A();
  const A.nome();
  const factory A.f() = A;
  A.nc();
}
class S {
  int y = 0;
}
class B extends S {
  const B();
}
mixin M {
  int z = 0;
}
class C with M {
  const C();
}
class D {
  abstract int w;
  static int s = 0;
  final int f = 0;
  const D();
}
class E {
  late final int l;
  external int e;
  const E();
}
```
```
const_constructor_with_non_final_field off=31 len=1 3:9 | Can't define a const constructor for a class with non-final fields.
const_constructor_with_non_final_field off=44 len=6 4:9 | Can't define a const constructor for a class with non-final fields.
const_constructor_with_non_const_super off=146 len=1 12:9 | A constant constructor can't call a non-constant super constructor of 'B'.
const_constructor_with_mixin_with_field off=203 len=1 18:9 | This constructor can't be declared 'const' because a mixin adds the instance field: 'M.z'.
concrete_class_with_abstract_member off=222 len=15 21:3 | 'w' must have a method body because 'D' isn't abstract.
const_constructor_with_non_final_field off=285 len=1 24:9 | Can't define a const constructor for a class with non-final fields.
late_final_field_with_const_constructor off=304 len=4 27:3 | Can't have a late final field in a class with a generative const constructor.
const_constructor_with_non_final_field off=348 len=1 29:9 | Can't define a const constructor for a class with non-final fields.
```
  `g03.dart`: `class A { var x; const A(this.x); const A.n() : x = 0, super(); }` → `off=27 len=1 3:9` e `off=46 len=3 4:9`;
  `abstract class B { abstract int y; const B(); }` → `off=117 len=1 8:9`; `class C { static var s; final f = 0; int get g => 0; set g(int v) {} const C(); }`
  → nenhum; `class D extends C { var z; const D() : super(); }` → `off=253 len=1 19:9`; `mixin M on C { var q; } class F extends C with M { const F(); }`
  → `const_constructor_with_mixin_with_field off=331 len=1 25:9` (`'M.q'`); `class G = C with M; class H extends G { const H(); }` →
  `const_constructor_with_non_const_super off=386 len=1 29:9` (`'H'`).
  `h04.dart` (3.6.2, sem o experimento): `class A { A(); augment const A(); }` → `expected_token off=19 len=7 3:3`,
  `missing_const_final_var_or_type off=19 len=7`, `const_constructor_with_non_final_field off=33 len=1 3:17`, `duplicate_constructor off=33 len=1`;
  `enum E { v; factory E.named() => v; } augment enum E { ; augment E.named(); }` → … `constructor_with_return_type off=105 len=7 11:3`,
  `duplicate_constructor off=113 len=7 11:11`, `non_const_generative_enum_constructor off=113 len=7 11:11`.

##### `const_type_parameter` (perda 9: FN 9, FP 0, msg 0, pos 0)
- **Emissão:** o motor, repassado por `_reportError` (está na LISTA) em qualquer chamador: (1) `ConstantVisitor._getConstantValue`,
  identificador que resolve para `TypeParameterElement` sem entrada no ambiente léxico de tipos, com `constructor_tearoffs`
  (`evaluation.dart:1845-1859`, no `errorNode2`); (2) `visitNamedType` com `isTypeLiteralInConstantPattern &&
  hasTypeParameterReference(type)` (`:1066-1072`; a extensão em `:3197-3201` exige `NamedType` ← `TypeLiteral` ← (um nó) ←
  `ConstantPattern`, isto é, o literal de tipo **parentizado**: `const (List<T>)`); (3) `SimpleIdentifier` com
  `tearOffTypeArgumentTypes` que mencionam parâmetro (`:1751-1756`); (4) convertido em `…_FUNCTION_TEAROFF` dentro de
  `visitFunctionReference` (`:887-894`).
- **Condição exata:** o parâmetro de tipo é **lido como valor** (`T` como expressão `Type`) numa expressão avaliada fora do ambiente de
  um construtor const: elemento de literal const, argumento de anotação, padrão constante, operando relacional, inicializador de
  `const` local. Dentro de inicializador de construtor const (`x = T`) não há avaliação pelo verificador → nada (f03 linha 8).
- **Posição:** o identificador `T`; no caso (2), o `NamedType` inteiro (`List<T>`, len 7).
- **Mensagem:** "Type parameters can't be used in a constant expression." Sem argumentos.
- **Supressões e ordem:** `case List<T>` **sem** parênteses → só `invalid_constant_pattern_generic` do parser (o literal de tipo
  avalia; e04 linha 19). `const <T>[]` é `invalid_type_argument_in_const_literal`, não este. `const [X]` num inicializador de
  construtor const sai (o `visitListLiteral` avalia fora do ambiente; h05 linha 4).
- **No DartForge:** o motor emite em `crates/types/src/constantes/avaliador.rs:1385`; `verificador.rs:40` repassa. FN:
  `const_type_parameter/…f1f17739.dart:2:14` (`case T`: padrão guardado como `PatternKind::Variable`, mesma causa de
  `constant_pattern_with_non_constant_expression`), `…46dae9bd.dart:2:21` (`const (List<T>)`: falta a regra (2) no motor),
  `metadata/type_parameter_scope_other_test.dart` (7: `@Annotation(T)` em parâmetros de tipo de classe, função, extensão, método,
  mixin e typedefs): o verificador não visita anotações (`verificador.rs::declaracao`, `:175-207`, não percorre `metadata`).
  Mudança: visitar anotações de declarações, membros, parâmetros e **parâmetros de tipo** (`visitAnnotation`, §1.3), com o `T` do
  argumento resolvido para o parâmetro de tipo da própria lista; regra (2) no avaliador.
- **Exemplos (oráculo vivo 3.6.2):** `e04.dart` (linhas 16–21, `void f<T, …>(…, Object? x)`):

| linha: entrada | diagnóstico |
|---|---|
| 16: `if (x case T) {}` | `const_type_parameter off=399 len=1 16:14 \| Type parameters can't be used in a constant expression.` |
| 17: `if (x case const (List<T>)) {}` | `off=425 len=7 17:21` |
| 18: `if (x case const (T)) {}` | `off=458 len=1 18:21` |
| 19: `if (x case List<T>) {}` | só `invalid_constant_pattern_generic off=482 len=1 19:18` |
| 20: `if (x case const <T>[]) {}` | `invalid_type_argument_in_const_literal off=510 len=1 20:21` |
| 21: `if (x case == T) {}` | `const_type_parameter off=535 len=1 21:17` |

  `c03.dart` (`class A { A(); const A.c(Object o); }`, `class G<T> { const G(Object o); }`):

```dart
class C<@A() T, @A.c(T) U, @G<T>(1) V> {
  void m<@A.c(X) X>() {}
  @A.c(T)
  int f = 0;
}
void f(@A() int p, [@A.c(q) int q = 0]) {
  @A() var loc = 0;
}
```
```
non_constant_annotation_constructor off=86 len=4 8:9 | Annotation creation can only call a const constructor.
const_type_parameter off=99 len=1 8:22 | Type parameters can't be used in a constant expression.
const_type_parameter off=133 len=1 9:15 | Type parameters can't be used in a constant expression.
const_type_parameter off=151 len=1 10:8 | Type parameters can't be used in a constant expression.
non_constant_annotation_constructor off=176 len=4 13:8 | Annotation creation can only call a const constructor.
const_with_non_constant_argument off=194 len=1 13:26 | Arguments of a constant creation must be constant expressions.
undefined_identifier off=194 len=1 13:26 | Undefined name 'q'.
non_constant_annotation_constructor off=213 len=4 14:3 | Annotation creation can only call a const constructor.
unused_local_variable off=222 len=3 14:12 | The value of the local variable 'loc' isn't used.
```
  (offsets contados no arquivo inteiro, que começa pelas duas classes.) `h05.dart` (`class A<X>`): inicializadores
  `x1 = const [X]`, `x3 = const {X}`, `x5 = const {X: null}` → `const_type_parameter` em `off=82`, `off=104`, `off=126` (len 1);
  `x7 = X` → nada; no corpo de `m()`: `const [X];` → `off=301 len=1 13:12`; `const y = X;` → `off=319 len=1 14:15`;
  `const z = (X, 1);` → `off=337 len=1 15:16`.

##### `const_constructor_with_field_initialized_by_non_const` (perda 3: FN 1, FP 2, msg 0, pos 0)
- **Emissão:** `ConstantVerifier._validateFieldInitializers` (`constant_verifier.dart:842-881`), de `visitConstructorDeclaration:182-188`.
- **Condição exata:** §1.3 — construtor const não factory; cada campo **não estático** com inicializador (final ou não; em enum,
  menos `values`) cuja avaliação (`ConstantVisitor` com relator nulo, **sem** ambiente de construtor) não dá valor.
- **Posição:** o token `const` do construtor (len 5).
- **Mensagem:** "Can't define the 'const' constructor because the field '{0}' is initialized with a non-constant value." `{0}` = nome.
- **Supressões e ordem:** um relato por construtor gerador const × campo. O erro específico do inicializador sai **também**, no
  próprio campo, por `visitVariableDeclaration` (`_reportError(result, null)`: só códigos da LISTA, só para campo `final` de classe
  com gerador const): `final int a = f()` → `const_eval_method_invocation` no `f()`; `final b = [1]` (genérico) → só este código;
  campo não final (`int d = f()`) → só este código (+ `const_constructor_with_non_final_field`).
- **No DartForge:** `verificador.rs:305-330` (`inicializadores_de_campo`), token por busca textual (`palavra_const`, `:360-373`).
  FN `const_with_type_parameters/…FunctionTearoff__4562b1b6.dart:3:3` (`final x = f<U>;`: o nosso motor avalia `TypeArguments` com
  parâmetro de tipo como valor; o oficial dá `…_FUNCTION_TEAROFF` → inválido; g04 campo `b`). FP `…ConstructorTearo_990aed62.dart:2:3`
  (`final x = A<T>.new;`: no oficial o tear-off de construtor **avalia** — g04 campo `a` não entra na lista — e só sai
  `const_with_type_parameters`; nós tratamos `.new` como acesso a propriedade). FP
  `primary_constructors/const/potentially_constant_error_test.dart:56:7` (3.13, não conferido).
- **Exemplos (oráculo vivo 3.6.2):** `b05.dart`:

```dart
int f() => 1;
class A {
  final int a = f();
  final b = [1];
  final int c = 2;
  static final int s = f();
  int d = f();
  const A();
  const A.nome();
  const factory A.fac() = A;
}
enum E {
  v;
  final int a = f();
  const E();
}
```
```
const_eval_method_invocation off=40 len=3 3:17 | Methods can't be invoked in constant expressions.
const_constructor_with_field_initialized_by_non_const off=126 len=5 8:3 | Can't define the 'const' constructor because the field 'a' is initialized with a non-constant value.
const_constructor_with_field_initialized_by_non_const off=126 len=5 8:3 | … the field 'b' …
const_constructor_with_field_initialized_by_non_const off=126 len=5 8:3 | … the field 'd' …
const_constructor_with_non_final_field off=132 len=1 8:9 | Can't define a const constructor for a class with non-final fields.
const_constructor_with_field_initialized_by_non_const off=139 len=5 9:3 | … the field 'a' …   (idem 'b' e 'd', mesmo offset)
const_constructor_with_non_final_field off=145 len=6 9:9 | Can't define a const constructor for a class with non-final fields.
const_eval_method_invocation off=216 len=3 14:17 | Methods can't be invoked in constant expressions.
const_constructor_with_field_initialized_by_non_const off=223 len=5 15:3 | Can't define the 'const' constructor because the field 'a' is initialized with a non-constant value.
```
  (as linhas com `…` têm a mesma mensagem da primeira, trocando o nome do campo.)

##### `non_constant_type_argument` (perda 3: FN 3, FP 0, msg 0, pos 0)
- **Emissão:** `FfiVerifier` (`analyzer/lib/src/generated/ffi_verifier.dart`), `FfiCode.NON_CONSTANT_TYPE_ARGUMENT`; não é do
  verificador de constantes. Nove pontos:
  | método (linha) | forma | condição | nó | `{0}` |
  |---|---|---|---|---|
  | `_validateTypeArgument` (`:2027-2035`) | argumento de tipo escrito (`asFunction<R>()`, `lookupFunction<…>`) | `typeArgument.type is TypeParameterType` | o argumento de tipo | o nome passado pelo chamador (`asFunction`…) |
  | `_validateAsFunction` (`:1203-1217`) | `p.asFunction<F>()` | alvo `Pointer<NativeFunction<X>>` com `X` parâmetro de tipo | o alvo (`p`) | `asFunction` |
  | `_validateRefPrefixedIdentifier` (`:1904-1914`), `_validateRefPropertyAccess` (`:1916-1926`) | `p.ref` / `(e).ref` | tipo do alvo não é tipo nativo FFI válido (`_isValidFfiNativeType(…, allowEmptyStruct: true)`: `Pointer<T>` com `T` parâmetro) | o acesso inteiro | `ref` |
  | `_validateRefIndexed` (`:1889-1900`) | `p[i]` | idem, `allowArray: true` | o `IndexExpression` | `[]` |
  | `_validateSizeOf` (`:1928-1942`) | `sizeOf<T>()` | argumento inferido/escrito não é tipo nativo válido | a invocação | `sizeOf` |
  | `_validateAllocate` (`:1127-1142`) | `alloc<T>()` | idem | a invocação | `AllocatorAlloc.call` |
  | `_validateCreate` (`:1381-1395`) | `Struct.create<T>()` / `Union.create<T>()` | idem | a invocação | `Struct.create` / `Union.create` |
  | `_validateElementAt` (`:1397-1410`) | `p.elementAt(i)` | `T` de `Pointer<T>` não é válido | a invocação | `elementAt` |
- **Mensagem:** "The type arguments to '{0}' must be known at compile time, so they can't be type parameters."
- **Supressões e ordem:** no `asFunction`, o teste do argumento escrito vem primeiro e retorna; o do alvo vem depois. `_isValidFfiNativeType`
  e os chamadores: não detalhados aqui (família C/FFI).
- **No DartForge:** não existe (nenhuma ocorrência de `NON_CONSTANT_TYPE_ARGUMENT` nem regra de `asFunction`/`ref`/`sizeOf` em
  `crates/analise` e `crates/types/src/inferencia`). FN: `non_constant_type_argument/…ref_typ_cf5f5ff8.dart:4:5` (`p.ref`, len 5),
  `…asFunction_R.dart:5:18` (o `R`), `non_native_function_type_argument_to_pointer/…5c69d119.dart:5:5` (o alvo `p`). Mudança: pertence
  ao verificador FFI (família C); a parte de constantes não a implementa.
- **Exemplos (oráculo vivo 3.6.2):** `h01.dart`:

```dart
import 'dart:ffi';
typedef F = int Function(int);
T genericRef<T extends Struct>(Pointer<T> p) => p.ref;
class C<R extends int Function(int), T extends Function> {
  void f(Pointer<NativeFunction<F>> p, Pointer<NativeFunction<T>> q) {
    p.asFunction<R>();
    q.asFunction<F>();
    p.asFunction<F>();
  }
}
void g<T extends Struct, U extends NativeType>(Pointer<T> p, Array<T> a) {
  p.refWithFinalizer;
  p[1];
  p + 1;
  sizeOf<T>();
  sizeOf<Int8>();
  sizeOf<U>();
  Pointer.fromFunction<T Function()>(g);
}
```
```
non_constant_type_argument off=98 len=5 3:49 | The type arguments to 'ref' must be known at compile time, so they can't be type parameters.
non_constant_type_argument off=252 len=1 6:18 | The type arguments to 'asFunction' must be known at compile time, so they can't be type parameters.
non_constant_type_argument off=262 len=1 7:5 | The type arguments to 'asFunction' must be known at compile time, so they can't be type parameters.
non_native_function_type_argument_to_pointer off=298 len=1 8:18 | Can't invoke 'asFunction' because the function signature 'NativeFunction<F>' for the pointer isn't a valid C function signature.
undefined_getter off=389 len=16 12:5 | The getter 'refWithFinalizer' isn't defined for the type 'Pointer<T>'.
non_constant_type_argument off=409 len=4 13:3 | The type arguments to '[]' must be known at compile time, so they can't be type parameters.
non_constant_type_argument off=426 len=11 15:3 | The type arguments to 'sizeOf' must be known at compile time, so they can't be type parameters.
non_constant_type_argument off=459 len=11 17:3 | The type arguments to 'sizeOf' must be known at compile time, so they can't be type parameters.
type_argument_not_matching_bounds off=466 len=1 17:10 | 'U' doesn't conform to the bound 'SizedNativeType' of the type parameter 'T'.
must_be_a_native_function_type off=495 len=12 18:24 | The type 'T Function()' given to 'fromFunction' must be a valid 'dart:ffi' native function type.
```
  `j03.dart` (`final class S extends Struct { external Pointer<Void> p; }`, `void g<T extends Struct>(Pointer<S> s, Allocator a, Pointer<T> p)`):
  `(p).ref;` → `off=170 len=7 8:3` (`'ref'`); `a<T>();` → `off=191 len=6 10:3` (`'AllocatorAlloc.call'`);
  `Struct.create<T>();` → `off=201 len=18 11:3` (`'Struct.create'`); `s.ref;`, `(s).ref;`, `a<S>();` → nenhum.

##### `non_constant_annotation_constructor` (perda 2: FN 2, FP 0, msg 0, pos 0)
- **Emissão:** `ConstantVerifier.visitAnnotation` (`constant_verifier.dart:103-115`).
- **Condição exata:** `node.element is ConstructorElement && !element.isConst` (construtor sem nome ou nomeado; padrão implícito
  inclusive).
- **Posição:** o nó `Annotation` inteiro: de `@` ao `)` (`@A.nc()` len 7; `@A()` len 4).
- **Mensagem:** "Annotation creation can only call a const constructor." Sem argumentos (e sem correção no `messages.yaml:11222`).
- **Supressões e ordem:** ao relatar, `return`: não checa `no_annotation_constructor_arguments` nem avalia os argumentos — mas a
  descida (`super.visitAnnotation`) já aconteceu, então criações/literais const **dentro** dos argumentos foram verificados. Em
  qualquer posição de anotação: declaração, membro, parâmetro, parâmetro de tipo, variável local (c03). O que a resolução não liga a
  construtor (`@f()`, `@n`, `@A.zz()`, `@Indef()`) fica com `invalid_annotation`/`undefined_annotation`.
- **No DartForge:** não existe; o verificador de constantes não visita anotações. A resolução de anotações existe
  (`crates/types/src/inferencia/funcoes.rs:986-1070`, `validar_anotacao`: `UNDEFINED_ANNOTATION`/`INVALID_ANNOTATION`), e é o ponto
  que conhece o construtor resolvido. FN: `non_constant_annotation_constructor/NonConstantAnnotationConstructor__named.dart:4:1` e
  `…__unnamed.dart:4:1`. Mudança: `visitAnnotation` no verificador (junto com `const_with_non_constant_argument` e
  `no_annotation_constructor_arguments`), lendo o construtor resolvido da anotação.
- **Exemplos (oráculo vivo 3.6.2):** `c02.dart`:

```dart
class A {
  const A();
  A.nc();
  const A.n(int x);
}
A f() => A.nc();
const k = A();
@A.nc()
int a = 0;
@A
int b = 0;
@A.n
int c = 0;
@A()
int d = 0;
@A.n(n)
int e = 0;
@f()
int g = 0;
@k
int h = 0;
@n
int i = 0;
@Indef()
int j = 0;
@A.zz()
int l = 0;
int n = 1;
```
```
non_constant_annotation_constructor off=87 len=7 8:1 | Annotation creation can only call a const constructor.
no_annotation_constructor_arguments off=106 len=2 10:1 | Annotation creation must have arguments.
no_annotation_constructor_arguments off=120 len=4 12:1 | Annotation creation must have arguments.
const_with_non_constant_argument off=157 len=1 16:6 | Arguments of a constant creation must be constant expressions.
invalid_annotation off=171 len=4 18:1 | Annotation must be either a const variable reference or const constructor invocation.
invalid_annotation off=201 len=2 22:1 | Annotation must be either a const variable reference or const constructor invocation.
undefined_annotation off=215 len=8 24:1 | Undefined name 'Indef' used as an annotation.
invalid_annotation off=235 len=7 26:1 | Annotation must be either a const variable reference or const constructor invocation.
```
  `j04.dart` (`class A { const A([Object? o]); A.nc(); }`, `int n = 0;`, e `enum E { a(0), b.n(n); final int x; const E(this.x); const E.n(this.x); }`):
  `@A(const A.nc())` → `const_with_non_const off=138 len=5 9:4` + `const_with_non_constant_argument off=138 len=5 9:4`;
  `@A([n])` → `const_with_non_constant_argument off=156 len=1 10:5` + `non_constant_list_element off=156 len=1`; `@A(A.nc)` → nenhum;
  constante de enum `b.n(n)` → `const_with_non_constant_argument off=21 len=1 2:13`.


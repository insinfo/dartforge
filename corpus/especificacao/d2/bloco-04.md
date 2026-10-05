#### 5. Os códigos

##### `constant_pattern_never_matches_value_type` (perda 52: FN 52, FP 0, msg 0, pos 0)
- **Emissão:** `ConstantVerifier.visitConstantPattern` (`constant_verifier.dart:148-156`), com `_canBeEqual` (`:517-546`); fase
  ConstantVerifier. Único emissor. Aviso (`WarningCode`).
- **Condição exata:** §4.1 — biblioteca com `patterns`; a expressão avalia; o **valor** tem igualdade primitiva (`double` e classes com
  `==`/`hashCode` próprios ficam de fora); `matchedValueType` gravado na resolução, com tipos de extensão apagados; `_canBeEqual` falso.
- **Posição:** o `ConstantPattern`: de `const` (se houver) ao fim da expressão; `case 1` → len 1; `const A()` → len 9; padrão
  parentizado `(true)` → só `true`.
- **Mensagem:** `The matched value type '{0}' can never be equal to this constant of type '{1}'.` `{0}` = tipo casado apagado, `{1}` =
  `value.type`; os dois são `DartType` → `getDisplayString` com alias (§0 T7); nomes iguais ganham o URI (regra geral do relator).
- **Supressões e ordem:** expressão com `InvalidType` → nada; avaliação inválida → só `constant_pattern_with_non_constant_expression`;
  ao relatar, **não desce** na expressão (criações const internas não são verificadas). Coexiste com `unreachable_switch_case` (token
  `case`), com `invalid_constant_pattern_*` do parser e com `pattern_never_matches_value_type` de outros padrões. Tipo casado `Never`
  (caso morto) ou `dynamic`/registro → nada.
- **No DartForge:** regra em `crates/types/src/inferencia/padroes.rs:356-387` (`constante_nunca_casa`, chamada em `:163` com o tipo
  casado `t`) e `pode_ser_igual` (`:390-424`) — na inferência, com o tipo **estático** da expressão, só para literais simples e enums
  (`:364-379`). O tipo casado é gravado em `UnitBodyTypes::tipos_casados` (`crates/types/src/resolved.rs:109`; `padroes.rs:121`),
  "ainda sem a promoção do fluxo de padrões". Causas das 52 FN:
  1. (44) `patterns/invalid_const_pattern_test.dart` (31) e `invalid_const_pattern_binary_test.dart` (13): constantes `const Class()`,
     `const <int>[]`, `const {}`, literais de tipo, `#a`, `!false`… sobre valor `int`: precisam do **valor avaliado** (tipo do valor e
     `hasPrimitiveEquality`), que só o verificador tem.
  2. (4) `constant_pattern_never_matches_value_type/*`: `b_412c86ec` (`T & int` × `bool`: o nosso `pode_ser_igual` devolve `true` para
     `Type::Intersection`; o oficial usa `promotedBound`), `c_72136293` (`A<int>` × `const A<num>()`), `c_896a9b96`/`c_cb0c58ee`
     (`B` × `const A()`): instâncias de classe, só com o valor.
  3. (4) `patterns/exhaustiveness/{bool_switch_test:165, enum_switch_test:249, null_type_test:33, object_pattern_switch_test:204}`:
     segundo `case null` — tipo casado promovido pelos casos anteriores (e01 linha 10, e05).
  Mudança: mover a regra para `crates/types/src/constantes/verificador.rs::padrao` (`:530-540`, depois de `avaliar_e_relatar`), com
  `Valor.tipo`, `Motor::igualdade_primitiva` (`avaliador.rs:1231`) e `tipos_casados`; retirar `constante_nunca_casa` da inferência;
  `pode_ser_igual` passa a tratar interseção pelo limite promovido; a inferência grava o tipo casado **promovido** (T6: `case null` e
  padrões de tipo anteriores, `&&`, variável promovida no `else`). Parar a descida ao relatar.
- **Exemplos (oráculo vivo 3.6.2):** `e01`, `e03`, `e04`, `e05` em §4.1; amostras `c_72136293` e `c_cb0c58ee` reduzidas (`j01.dart`):

```dart
void f(A<int> x, B y) {
  if (x case const A<num>()) {}
  if (y case const A<int>()) {}
}
class A<T> { const A(); }
class B extends A<int> { const B(); }
```
```
constant_pattern_never_matches_value_type off=37 len=14 2:14 | The matched value type 'A<int>' can never be equal to this constant of type 'A<num>'.
constant_pattern_never_matches_value_type off=69 len=14 3:14 | The matched value type 'B' can never be equal to this constant of type 'A<int>'.
```

##### `const_with_type_parameters` (perda 39: FN 39, FP 0, msg 0, pos 0)
Nome publicado de três códigos: `CONST_WITH_TYPE_PARAMETERS`, `…_CONSTRUCTOR_TEAROFF`, `…_FUNCTION_TEAROFF` (`messages.yaml:3326`, `:3375`, `:3381`).
- **Emissão:** `ConstantVerifier`: `visitInstanceCreationExpression:256-257`, `visitConstructorReference:197-199`,
  `visitFunctionReference:228-236`, `visitGenericFunctionType:245-249`, todos por `_checkForConstWithTypeParameters` (`:554-607`); e o
  motor, `ConstantVisitor.visitFunctionReference` (`evaluation.dart:862-876` instanciação implícita; `:885-894` argumento escrito que
  avalia para `CONST_TYPE_PARAMETER` é convertido), repassado por `_reportError` (está na LISTA).
- **Condição exata:**
  ```
  _checkForConstWithTypeParameters(tipo, código, permitidos = {}):
    NamedType: elemento é TypeParameterElement e ∉ permitidos → R no NamedType; return
               senão, recursão em cada argumento de tipo
    GenericFunctionType: permitidos += parâmetros de tipo do próprio tipo; recursão nos limites deles, no retorno e no tipo de cada
               SimpleFormalParameter
    RecordTypeAnnotation: nada (não é visitado)
  ```
  | forma | quando | código | nó |
  |---|---|---|---|
  | criação `const C<T>()` / implícita em contexto const | `node.isConst` | `CONST_WITH_TYPE_PARAMETERS` | o `NamedType` do parâmetro |
  | `C<T>.new`, `C<T>.nome` | `inConstantContext \|\| inConstantExpression` (§1.2) | `…_CONSTRUCTOR_TEAROFF` | idem |
  | `f<T>` com argumentos escritos | idem | `…_FUNCTION_TEAROFF` | idem (cada argumento) |
  | `x is F`/`x as F` com `F` tipo de função genérico escrito | o `is`/`as` está `inConstantContext` | `CONST_WITH_TYPE_PARAMETERS` | o `NamedType` |
  | `f` sem argumentos, instanciado pelo contexto com tipo que menciona parâmetro (depois de aplicar o ambiente léxico de tipos) | qualquer avaliação da expressão | `…_FUNCTION_TEAROFF` | a referência à função inteira (`id`, `prefix.f1`, `c01`) |
- **Posição:** o `NamedType` (`T`, `T?` com o `?`: len 2); na instanciação implícita, o nó `FunctionReference`.
- **Mensagem:** "A constant creation can't use a type parameter as a type argument." / "A constant constructor tearoff can't use…" /
  "A constant function tearoff can't use…". Sem argumentos.
- **Supressões e ordem:** o teste da criação vem **antes** da avaliação e não a impede: `const A<T>.nao_existe()` dá este código **e**
  `const_with_undefined_constructor`; `const A<T>.nc()` dá este **e** `const_with_non_const` (a09). Em campo de classe **sem**
  construtor gerador const, em enum e fora de contexto constante, `G<T>.new` e `id<T>` não dão nada (g04, classe `L` e enum `En`). Em
  inicializador de construtor const (`x = A<T>.a`, `x = idf<T>`) também nada (não é contexto nem "expressão constante"; f03). O relato
  do verificador e o do motor no mesmo `T` coincidem (um só). Anotação `@G<T>(1)` não é `InstanceCreationExpression`: nada (c03).
- **No DartForge:** não existe (busca por `CONST_WITH_TYPE_PARAMETERS` em `crates/`: só a tabela `ESPECIFICOS`,
  `verificador.rs:41`). `verificador.rs::expr` (`:590-727`) não olha argumentos de tipo; o motor não avalia `TypeArguments` com
  parâmetro. As 39 FN: `const/instantiated_function_constant_error_test.dart` (18: `f1<Z>`, `prefix.f2<Z>`, `c01<Z>`, instanciação
  implícita), `const/constant_type_variable_error_test.dart:37:20` (`const B<X>()` em inicializador), `const_with_type_parameters/*`
  (20: criação direta/indireta, tipos de função, tear-offs em valor padrão e em campo). Mudança: em `verificador.rs::expr`, os quatro
  ramos acima, com um conjunto de parâmetros permitidos e uma função "este `ast::TypeId` nomeia um parâmetro de tipo em alcance" (a
  inferência precisa expor a resolução do tipo escrito, ou reconstruir o alcance pelo span da declaração); `inConstantExpression` como
  dois bits passados na descida (valor padrão; inicializador de campo de instância de classe com gerador const); no motor, o caso da
  instanciação implícita (`UnitBodyTypes` já guarda os argumentos inferidos do tear-off — não verificado em que campo).
- **Exemplos (oráculo vivo 3.6.2):** `a08.dart`:

```dart
class G<T> {
  const G();
}
U id<U>(U u) => u;
class K<T> {
  m() {
    const a = G<T>.new;
    const int Function(int) b = id;
    const c = id<T>;
    const T Function(T) d = id;
    const e = 1 is void Function<X>(T, X);
    const f = 1 as T Function();
    return [a, b, c, d, e, f];
  }
}
```
```
const_with_type_parameters off=84 len=1 7:17 | A constant constructor tearoff can't use a type parameter as a type argument.
const_with_type_parameters off=145 len=1 9:18 | A constant function tearoff can't use a type parameter as a type argument.
const_with_type_parameters off=177 len=2 10:29 | A constant function tearoff can't use a type parameter as a type argument.
const_with_type_parameters off=217 len=1 11:37 | A constant creation can't use a type parameter as a type argument.
variable_type_mismatch off=238 len=17 12:15 | A value of type 'int' can't be assigned to a const variable of type 'T Function()'.
const_with_type_parameters off=243 len=1 12:20 | A constant creation can't use a type parameter as a type argument.
```
  `g04.dart` (campos e valores padrão; `class K<T>` tem `const K();`):

| linha: entrada | diagnósticos |
|---|---|
| 6: `final a = G<T>.new;` | `const_with_type_parameters off=74 len=1 6:15` (constructor tearoff) |
| 7: `final b = id<T>;` | `const_with_type_parameters off=97 len=1 7:16` (function tearoff) |
| 8: `final int Function(int) b2 = id;` | nenhum |
| 9: `final T Function(T) b3 = id;` | `const_with_type_parameters off=163 len=2 9:28` (function tearoff, no `id`) |
| 10: `final c = const G<T>();` | `const_with_type_parameters off=187 len=1 10:21` (creation) |
| 11: `final d = G<T>();` | nenhum deste código |
| 13: `const K();` | `const_constructor_with_field_initialized_by_non_const off=243 len=5 13:3`, três vezes: campos `'b'`, `'b3'`, `'d'` (`a` e `c` avaliam) |
| 14: `void m({Object p = G<T>.new, Object q = id<T>, Object r = const G<T>()}) {}` | `off=277 len=1 14:24` (constructor tearoff), `off=299 len=1 14:46` (function tearoff), `off=322 len=1 14:69` (creation); **sem** `non_constant_default_value` |
| 17–19: classe `L<T>` (só `const factory`): `final a = G<T>.new; final b = id<T>; final T Function(T) b3 = id;` | nenhum |
| 29–33: `enum En<T> { v<int>(); final a = G<T>.new; const En(); }` | sem este código; `const_eval_throws_exception off=578 len=8 30:3` na constante `v<int>()` (ctx: desencontro de tipo do campo `a`; motor, D.R4-1) |

  `h06.dart` (`void f1<T extends num>(T t) {}`, `const c01 = f1;`, `import 'h06.dart' as prefix;`, dentro de `Cl<T extends num>.test<Z extends num>()`):

| linha: entrada | diagnóstico (`const_with_type_parameters`, function tearoff) |
|---|---|
| 6: `const c03 = f1<Z>;` | `off=152 len=1 6:20` |
| 7: `const c04 = prefix.f1<Z>;` | `off=182 len=1 7:27` |
| 8: `const c07 = prefix.c01<Z>;` | `off=213 len=1 8:28` |
| 9: `const c08 = c01<T>;` | `off=237 len=1 9:21` |
| 10: `const void Function(Z) c09 = f1;` | `off=274 len=2 10:34` (o `f1`) |
| 11: `const void Function(T) c10 = prefix.f1;` | `off=311 len=9 11:34` (`prefix.f1`) |
| 12: `const void Function(Z) c11 = c01;` | `off=355 len=3 12:34` (`c01`) |
| 13: `const c12 = f1<List<Z>>;` | `off=384 len=1 13:25` (o `Z` interno) + `type_argument_not_matching_bounds off=379 len=7` |

##### `const_with_non_const` (perda 37: FN 31, FP 6, msg 0, pos 0)
- **Emissão:** três caminhos com o mesmo resultado: (1) `ErrorVerifier._checkForConstWithNonConst` (`error_verifier.dart:2993-3008`), de
  `visitInstanceCreationExpression:1112-1113` quando `node.isConst` e o tipo do `NamedType` é `InterfaceType`; (2) o motor,
  `_InstanceCreationEvaluator.evaluate` (`evaluation.dart:3078-3088`), cujo `InvalidConstant` o
  `ConstantVerifier.visitInstanceCreationExpression` relata **direto** (`:272-282`); (3) o mesmo `InvalidConstant` chegando por
  `_reportError` de um chamador (variável const, elemento de literal, campo de registro…), onde é **substituído** pelo padrão.
- **Condição exata:** `constructorName.staticElement != null && !staticElement.isConst`. `isConst` de construtores sintéticos:
  o construtor padrão implícito **não** é const; os encaminhados de `class B = S with M;` têm
  `isConst = superCtor.isConst && !mixins.any(m => m.element.fields.any(f => !f.isSynthetic))` (`element.dart:595-596`, `:613-615`) —
  **qualquer** campo declarado no mixin, inclusive `static`, tira o const; getter declarado não (o campo dele é sintético).
- **Posição:** o token `const` (len 5) se escrito; senão o nó da criação inteira (`NaoConst()`, `P.nc(1)`).
- **Mensagem:** "The constructor being called isn't a const constructor." Sem argumentos.
- **Supressões e ordem:**
  | situação | o que sai |
  |---|---|
  | `var x = const NaoConst();` | só `const_with_non_const` no `const` |
  | `const x = NaoConst();` | `const_with_non_const` **e** `const_initialized_with_non_constant_value`, os dois no nó da criação |
  | `const z = [NaoConst()];` | os dois acima + `non_constant_list_element`, mesmo nó (a04 linha 7); em mapa `non_constant_map_key`, em registro `non_constant_record_field` (i02) |
  | `const [new P()]` | só `non_constant_list_element` no `new P()` (não é `isConst`; i02 linha 8) |
  | `const P.nc(n())` (argumento não constante) | só `const_with_non_const` (o motor para antes dos argumentos; a11 linha 8) |
  | `const A<T>.nc()` | + `const_with_type_parameters` (a09) |
  | classe abstrata com construtor const gerador | só `instantiate_abstract_class`; enum: `invalid_reference_to_generative_enum_constructor` (a10) |
  | typedef para classe sem construtor const | `const_with_non_const` no `const` (a10 linha 11) |
  | construtor não resolvido | `const_with_undefined_constructor(_default)`, nunca este |
- **No DartForge:** só o caminho do motor: `crates/types/src/constantes/avaliador.rs:1702-1717` (`avaliar_chamada`: `!fe.const_` →
  `CONST_WITH_NON_CONST` na palavra ou no nó), relatado por `verificador.rs:859-884` (`criacao_constante`, que **desiste** sem
  `Resolved::Constructor`, `:860`).
  - FN (31): `mixin_constructor_forwarding/const_constructor*_error_test.dart` (30) e
    `const_with_non_const/ConstWithNonConst__mixinApplication_con_af2c44f3.dart:8:9`: construtor encaminhado de aplicação de mixin fica
    sem resolução na inferência (`crates/types/src/inferencia/chamadas.rs:1346`), e o verificador desiste. Mudança: resolver o
    encaminhado (ou, no verificador, criação const sem resolução cuja classe é `ClassKind::MixinApplication`: construtor homônimo da
    superclasse) e aplicar a regra de `isConst` acima, com os campos declarados dos mixins (`ClassElement::mixin_classes`, `fields`,
    `crates/elements/src/model.rs:306`, `:320`), estáticos inclusive.
  - FP (6): `const_eval_throws_exception/*fromEnvironme*` (5 linhas em 4 arquivos): `bool/int/String.fromEnvironment` vistos pelo patch
    do SDK sem `const` (§0 T3). `experimental_member_use/ExperimentalMemberUse__incorrectlyNeste_0e44e3bf.dart:16:11`: parâmetros com
    erro de sintaxe (`{this.y = false}` aninhado); o oráculo vivo (h02) dá só `final_not_initialized_constructor`, `expected_token`
    e `missing_identifier` — o construtor continua const. A causa do nosso lado (recuperação do parser perdendo o `const`, ou outro
    construtor escolhido): não verificado.
- **Exemplos (oráculo vivo 3.6.2):** `a04.dart`:

```dart
class NaoConst {
  NaoConst();
  NaoConst.n();
}
const x = NaoConst();
const y = const NaoConst.n();
const z = [NaoConst()];
```
```
const_initialized_with_non_constant_value off=59 len=10 5:11 | Const variables must be initialized with a constant value.
const_with_non_const off=59 len=10 5:11 | The constructor being called isn't a const constructor.
const_initialized_with_non_constant_value off=81 len=5 6:11 | Const variables must be initialized with a constant value.
const_with_non_const off=81 len=5 6:11 | The constructor being called isn't a const constructor.
const_initialized_with_non_constant_value off=112 len=10 7:12 | Const variables must be initialized with a constant value.
const_with_non_const off=112 len=10 7:12 | The constructor being called isn't a const constructor.
non_constant_list_element off=112 len=10 7:12 | The values in a const list literal must be constants.
```
  `h07.dart` (aplicações de mixin; `class S { const S(); const S.n(int x); S.nc(); }`, `mixin M0 {}`, `mixin MF { final int f = 0; }`,
  `mixin MS { static int s = 0; }`, `mixin MG { int get g => 0; }`):

| linha: entrada | diagnósticos |
|---|---|
| 15: `const a = A0();` (`A0 = S with M0`) | nenhum |
| 16: `const b = AF();` (`S with MF`) | `const_with_non_const off=297 len=4 16:11` + `const_initialized_with_non_constant_value off=297 len=4` |
| 17: `const c = AS();` (`S with MS`, campo estático) | `const_with_non_const off=313 len=4 17:11` + `const_initialized_with_non_constant_value off=313 len=4` |
| 18: `const d = AG();` (`S with MG`, só getter) | nenhum |
| 19: `const e = A2.n(1);` (`S with M0, MF`) | `const_with_non_const off=345 len=7 19:11` + `const_initialized_with_non_constant_value off=345 len=7` |
| 20: `var f = const A0.nc();` | `const_with_non_const off=362 len=5 20:9` |
| 21: `var g = const AF.n(1);` | `const_with_non_const off=385 len=5 21:9` |

  `a02.dart`: `var x = const NaoConst();` (linha 4) → `const_with_non_const off=41 len=5 4:9`. `a07.dart`:
  `import 'dart:async' as a; … var z = const a.Completer();` → `const_with_non_const off=119 len=5 5:9` (factory não const do SDK).

##### `const_not_initialized` (perda 29: FN 29, FP 0, msg 0, pos 0)
- **Emissão:** dois emissores, mesmo relato:
  1. parser — `parseFieldInitializerOpt` (`_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3942-3947`): sem `=`, com
     `varFinalOrConst == const` e nome não sintético → `templateConstFieldWithoutInitializer`, convertido em
     `CompileTimeErrorCode.CONST_NOT_INITIALIZED` (`analyzer/lib/src/fasta/error_converter.dart:77-85`). Vale para **campos** (de
     classe, mixin, enum, extensão, extension type; estáticos ou não; `external`, `abstract`, `late`) e **variáveis de topo** (as duas
     chamadas em `:3718`, `:3730`). Locais usam `parseVariableInitializerOpt` (`:3964`), que não relata.
  2. `ErrorVerifier._checkForFinalNotInitialized` (`error_verifier.dart:3576-3617`): chamado em `visitTopLevelVariableDeclaration`
     (`:1558`), `visitVariableDeclarationStatement` (`:1638`) e, só para classes **sem** construtor gerador declarado, em
     `_checkForFinalNotInitializedInClass` (`:3625-3645`).
- **Condição exata:** lista `const`, variável sem inicializador. No verificador: `!_isInNativeClass && !list.isSynthetic`.
- **Posição:** o nome da variável (`atToken(variable.name)`; no parser, o token do nome). Uma por variável sem inicializador
  (`const int a, b = 1, c;` → `a` e `c`).
- **Mensagem:** "The constant '{0}' must be initialized." `{0}` = lexema do nome.
- **Supressões e ordem:** `for (const i; ;)` → nada (não é `VariableDeclarationStatement`); `for (const j in …)` →
  `for_in_with_const_variable`. Coexiste com `const_instance_field` (token `const`), `conflicting_modifiers` (`late const`),
  `concrete_class_with_abstract_member`, `unused_local_variable`.
- **No DartForge:** não existe (busca por `CONST_NOT_INITIALIZED` em `crates/`: só o comentário de
  `crates/frontend/src/parser/statements.rs:343`, que deixa o erro "para a fase seguinte"). As 29 FN são todas a regra ausente:
  `variable_not_initialized/*` (20, uma por espécie de declaração), `const/syntax_test.dart` (4), `const/const_locals_constant_locals_test.dart:10:9`,
  `static/final_field2_test.dart:13:16`. Mudança: uma função no verificador de constantes (ou em `crates/analise/src/inicializacao.rs`,
  que já percorre campos e topo), para topo, campos de qualquer declaração e `StmtKind::Variables` locais — não para o `for`.
- **Exemplos (oráculo vivo 3.6.2):** `c01.dart`:

| linha: entrada | diagnósticos |
|---|---|
| 1: `const x;` | `const_not_initialized off=6 len=1 1:7 \| The constant 'x' must be initialized.` |
| 2: `const int a, b = 1, c;` | `off=19 len=1 2:11` (`'a'`), `off=29 len=1 2:21` (`'c'`) |
| 3: `external const int ext;` | `off=51 len=3 3:20` |
| 4: `late const int lt;` | `conflicting_modifiers off=61 len=5 4:6` + `const_not_initialized off=71 len=2 4:16` |
| 6: `static const y;` (classe `K`) | `off=100 len=1 6:16` |
| 7: `const z;` (campo de instância) | `const_instance_field off=105 len=5 7:3` + `const_not_initialized off=111 len=1 7:9` |
| 8: `static const int p = 0, q;` | `off=140 len=1 8:27` |
| 9: `external static const int es;` | `off=171 len=2 9:29` |
| 10: `abstract const int ab;` | `concrete_class_with_abstract_member off=177 len=22` + `const_instance_field off=186 len=5 10:12` + `const_not_initialized off=196 len=2 10:22` |
| 13, 16, 20, 23: `static const m;` em mixin / `e` em extensão / `s` em enum / `t` em extension type | `off=228`, `off=270`, `off=305`, `off=351`, todos `len=1`, coluna 16 |
| 26: `const l;` (local) | `off=378 len=1 26:9` + `unused_local_variable` |
| 27: `const int m, n = 2;` | `off=393 len=1 27:13` |
| 28: `for (const i; ;) {}` | só `unused_local_variable off=416` |
| 29: `for (const j in []) {}` | `for_in_with_const_variable off=432 len=5 29:8` |

##### `const_with_undefined_constructor` (perda 25: FN 25, FP 0, msg 0, pos 0)
- **Emissão:** `ErrorVerifier._checkForConstWithUndefinedConstructor` (`error_verifier.dart:3020-3043`), de
  `visitInstanceCreationExpression:1112-1115`: `node.isConst` e o tipo do `NamedType` é `InterfaceType`. Fase ErrorVerifier.
- **Condição exata:** `constructorName.staticElement == null` e `constructorName.name != null`.
- **Posição:** o identificador do nome do construtor (`nao_existe`, len do nome).
- **Mensagem:** "The class '{0}' doesn't have a constant constructor '{1}'." `{0}` = `namedType.qualifiedName`
  (`analyzer/lib/src/dart/ast/extensions.dart:302-309`: `prefixo.Nome` se há prefixo de importação, senão o lexema escrito — o **nome
  do typedef** quando escrito por alias; sem argumentos de tipo); `{1}` = o nome escrito.
- **Supressões e ordem:** só existe para criação com `const` **escrito** resolvida como criação: `const d = P.zz(1)` (implícita) é
  `MethodInvocation` → `undefined_method` + `const_initialized_with_non_constant_value` (a11). Nome de tipo não resolvido →
  `creation_with_non_type`. O `ConstantVerifier` não avalia (sem construtor) nem desce nos argumentos. Soma-se a
  `const_with_type_parameters` (a09) e, em mixin, a `mixin_instantiate` (a10).
- **No DartForge:** não existe para a criação comum (o atalho de ponto marca o caso como "do caminho comum":
  `crates/types/src/inferencia/atalhos.rs:260-263`; o caminho `new` tem `NEW_WITH_UNDEFINED_CONSTRUCTOR` em
  `crates/types/src/inferencia/expr.rs:1793-1795`). FN: 5 em `const_with_undefined_constructor/*` (classe com nome, prefixo
  `a.Future`, enum com método/constante homônimos, enum sem o nome) e 20 em `dot_shorthands/equality/equality_ctor_error_test.dart`
  (oráculo 3.13: `const .nome(…)`; fora do 3.6.2 — não verificado aqui). Mudança: no ponto de `expr.rs` que relata
  `NEW_WITH_UNDEFINED_CONSTRUCTOR`, quando a criação tem `const` escrito relatar este código (nome escrito do tipo, com prefixo).
- **Exemplos (oráculo vivo 3.6.2):** `a01.dart`:

```dart
class A {
  const A();
}
var x = const A.nao_existe();
```
```
const_with_undefined_constructor off=41 len=10 4:17 | The class 'A' doesn't have a constant constructor 'nao_existe'.
```
  `a07.dart` (`import 'dart:async' as a; import 'dart:core' as c;`): `var x = const a.Future.foo();` →
  `const_with_undefined_constructor off=74 len=3 3:24 | The class 'a.Future' doesn't have a constant constructor 'foo'.`;
  `var y = const c.Object.bar();` → `off=104 len=3 4:24 | The class 'c.Object' doesn't have a constant constructor 'bar'.`
  `h03.dart` (`enum E { v; void foo() {} }`, `class A { const A.name(); }`, `typedef B = A;`):

| linha: entrada | diagnóstico |
|---|---|
| 2: `const E.foo();` | `const_with_undefined_constructor off=21 len=3 2:11 \| The class 'E' doesn't have a constant constructor 'foo'.` |
| 3: `const E.v();` | `off=38 len=1 3:11 \| … 'E' … 'v'.` |
| 4: `const E.bar();` | `off=53 len=3 4:11 \| … 'E' … 'bar'.` |
| 5: `const E();` | `invalid_reference_to_generative_enum_constructor off=68 len=1 5:9` |
| 6: `const B();` | `const_with_undefined_constructor_default off=81 len=1 6:9 \| The class 'B' doesn't have an unnamed constant constructor.` |
| 7: `const B.name();` | nenhum |
| 8: `const B.zz();` | `const_with_undefined_constructor off=114 len=2 8:11 \| The class 'B' doesn't have a constant constructor 'zz'.` |

  `a09.dart` (`class A<X> { const A(); A.nc(); }`, dentro de `K<T>.m`): `var a = const A<T>.nao_existe();` →
  `const_with_type_parameters off=79 len=1 7:21` + `const_with_undefined_constructor off=82 len=10 7:24`; `var b = const A<T>.nc();` →
  `const_with_non_const off=108 len=5 8:13` + `const_with_type_parameters off=116 len=1 8:21`.
  `a11.dart` (`class P { final int v; const P(this.v); P.nc(this.v); }`, `int n() => 1;`):

| linha: entrada | diagnósticos |
|---|---|
| 7: `var a = const P(n());` | `const_eval_method_invocation off=92 len=3 7:17 \| Methods can't be invoked in constant expressions.` |
| 8: `var b = const P.nc(n());` | `const_with_non_const off=106 len=5 8:9` |
| 9: `var c = const P.zz(n());` | `const_with_undefined_constructor off=139 len=2 9:17` (nada sobre `n()`) |
| 10: `const d = P.zz(1);` | `const_initialized_with_non_constant_value off=158 len=7 10:11` + `undefined_method off=160 len=2 10:13` |
| 11: `const e = [P.nc(1), P.zz(2)];` | `const_initialized_with_non_constant_value off=178 len=7`, `const_with_non_const off=178 len=7`, `non_constant_list_element off=178 len=7` (11:12); `non_constant_list_element off=187 len=7 11:21` + `undefined_method off=189 len=2` |

##### `const_with_undefined_constructor_default` (perda 2: FN 2, FP 0, msg 0, pos 0)
- **Emissão:** o mesmo método, ramo `name == null` (`error_verifier.dart:3036-3042`).
- **Condição exata:** criação com `const` escrito, tipo `InterfaceType`, sem nome de construtor, `staticElement == null` (a classe só
  tem construtores nomeados; ou é mixin).
- **Posição:** o `ConstructorName` inteiro — sem nome de construtor é o `NamedType` (com prefixo e argumentos de tipo se escritos).
- **Mensagem:** "The class '{0}' doesn't have an unnamed constant constructor." `{0}` = `namedType.qualifiedName`.
- **Supressões e ordem:** em mixin sai junto com `mixin_instantiate` (a10 linha 9: `const Mx()` → os dois em `off=145 len=2`).
- **No DartForge:** não existe; mesma mudança do código anterior (ramo sem nome). FN: `ConstWithUndefinedConstructor__class_no_1544e351.dart:6:16`
  (`typedef B = A; const B()` → nome `'B'`) e `…__class_unnamed.dart:5:16`.
- **Exemplos (oráculo vivo 3.6.2):** `a03.dart`:

```dart
class B {
  const B.nome();
}
var x = const B();
```
```
const_with_undefined_constructor_default off=44 len=1 4:15 | The class 'B' doesn't have an unnamed constant constructor.
```


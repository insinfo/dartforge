
#### 6. Códigos

Convenção: "§D" = o texto da rodada 3 (`ANALYZER-ESPECIFICACAO.md`, família D); o que ele já diz certo não é
repetido. Lado DartForge: linhas do que está **no disco** hoje — as funções `parametro_de_tipo`,
`instanciacao_implicita`, `expr_const`, `const_nao_inicializada` citadas em "§D Estado do código" **não
existem** no disco (foram revertidas no commit `fa8cfa96`).

##### `const_eval_throws_exception` (perda 21: FN 20, FP 1, msg 0, pos 0)
- **Emissão:** motor. (1) conversão de erro "de execução" na saída de uma criação:
  `evaluateAndFormatErrorsInConstructorCall` (`evaluation.dart:361-373`); (2) direto:
  `castToType` (`value.dart:350-354`), aritmética/deslocamento/`!null`/`convertToBool` de `null` (§2.3, §4),
  `fromEnvironment`/`Symbol` com argumentos errados (`evaluation.dart:2492-2495`, `:2524-2527`), campo
  inicializado duas vezes (`:2712-2717`, `:2941-2944`), getter sem variável (`:2722-2730`). Relatado pelo
  verificador (código específico, §5) ou cru pelo caminho 2.
- **Condição exata:** ver §3.4 (o que tem `isRuntimeException`) e a tabela de §2.3 (linhas
  `CONST_EVAL_THROWS_EXCEPTION`).
- **Posição:** (1) o nó da criação (`configuration.errorNode(node)`): a `InstanceCreationExpression` inteira,
  com `const`/prefixo/argumentos; a `EnumConstantDeclaration` inteira (`v(a)`, nome + argumentos; sem
  argumentos, só o nome); a `Annotation` inteira. (2) o nó da operação (binária inteira, `x as T` inteiro) — ou,
  se dentro de construtor, o nó da criação por `copyWithEntity`.
- **Mensagem:** `Evaluation of this constant expression throws an exception.` Contextos: (1) sempre termina com
  `The exception is '<mensagem do erro original>' and occurs here.` no intervalo original, precedida pelas
  "The evaluated constructor…" acumuladas; (2) dentro de construtor: `The error is in the field
  initializer/assert initializer/super constructor invocation of '<ctor>', and occurs here.`; fora: nenhum.
- **Supressões e ordem:** não sai se a criação usa construtor em ciclo (valor desconhecido) ou se um argumento
  anterior é inválido não-resolvido de forma que a checagem é pulada (`isInvalid`). Em variável `const` sai uma
  vez (os dois caminhos de §5 produzem o mesmo erro: deduplicado). Convive com o erro estático correspondente
  (`argument_type_not_assignable`, `field_initialized_in_parameter_and_initializer`).
- **No DartForge:** conversão em `avaliador.rs:590-599` (`formatar_erro_de_construtor`: troca o erro pelo novo
  **sem** contextos — não há `contextMessages` em `Invalida`, `avaliador.rs:29-37`); `As` em `:526-540`.
  Causas dos FN: (a) constantes de enum — `enum_construc_9a1a67d6`, `enum_int_String`, `enum_int_null`,
  `VariableNotInitialized__enum_instanceFi_{40eca8dc,48d9ffd8,8ff27e89,eee28126}` (7): `valor_de_enum`
  (`avaliador.rs:1540-1561`) monta só `index`/`_name`, não chama o construtor; (b) `asExpression__028c9aee`,
  `__76708aa4` (4) e `nnbd/const/potentially_constant_types_error_test.dart:23…39` (5): o ramo `As`
  (`:531-533`) desiste quando o tipo **estático** do `as` menciona parâmetro de tipo, sem aplicar `cx.tipos`
  (o oficial substitui antes, §4.3); (c) `fromEnvironme_55ccf6db`, `_d3d1fbc1` (3): causa conforme §D
  `const_with_non_const` (patch do SDK), não reconferida; (d) `ConstConstructorFieldTypeMismatchContex_0a10b92c`
  (1): `avaliador.rs:1937-1939` calcula `menciona_parametro` sobre o tipo **já substituído**; o oficial usa
  `field.type` declarado. FP `const/syntax_test.dart:109:20`: `static const X = const C1()` com `C1() : x =
  C0.X` — sem grafo, o construtor é avaliado; o oficial marca `C1` fora de ciclo-livre e dá valor desconhecido
  (`s12`). Mudança: grafo (§1.4), avaliação do construtor das constantes de enum com `_errorNode` = a
  declaração, substituição no `as`/`is`, tipo declarado em `_checkFields`, contextos em `Invalida`.
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  class C<T> {
    final t;
    const C(dynamic x) : t = x as List<T>;
  }
  main() {
    const C<int>(<int>[]);
    const C<int>(<num>[]);
    const C<int>(null);
  }
  ```
  - `const_eval_throws_exception off=103 len=21 7:3`, `ctx off=51 len=12 3:28 | The error is in the field initializer of 'C', and occurs here.`
  - `const_eval_throws_exception off=128 len=18 8:3`, mesmo contexto
  ```dart
  const dynamic a = '0';
  enum E {
    v(a);
    const E(int a);
  }
  ```
  - `const_eval_throws_exception off=34 len=4 3:3`, `ctx off=36 len=1 3:5 | The exception is 'A value of type 'String' can't be assigned to a parameter of type 'int' in a const constructor.' and occurs here.`
  ```dart
  enum A {
    e(0);
    final int v;
    const A(this.v) : v = 0;
  }
  ```
  - `const_eval_throws_exception off=11 len=4 2:3` (sem contexto: campo duas vezes) + `field_initialized_in_parameter_and_initializer off=52 len=1 4:21`
  ```dart
  var b1 = const bool.fromEnvironment(1);
  var b2 = const bool.fromEnvironment('x', defaultValue: 1);
  ```
  - `const_eval_throws_exception off=9 len=29 1:10` + `argument_type_not_assignable off=36 len=1`; `const_eval_throws_exception off=49 len=48 2:10` + `argument_type_not_assignable off=95 len=1`
  ```dart
  const num n = 1;
  const a = n as String;
  const Object o = 'x';
  const g = (o as int) + 1;
  ```
  - `const_eval_throws_exception off=27 len=11 2:11` (o `as` inteiro); `off=73 len=8 4:12` (`o as int`, sem os parênteses)
  ```dart
  class A {
    const A() : assert(false);
  }
  class B {
    const B(int x) : assert(x > 0, 'must be positive');
  }
  class C {
    const C(int x) : assert(x > 0, x);
  }
  const a = const A();
  const b = const B(0);
  const c = const C(0);
  var d = const B(-1);
  ```
  - `off=166 len=9 10:11`, `ctx off=24 len=13 2:15 | The exception is 'The assertion in this constant expression failed.' and occurs here.`
  - `off=187 len=10 11:11`, `ctx off=70 len=33 5:20 | The exception is 'An assertion failed with message 'must be positive'.' and occurs here.`
  - `off=209 len=10 12:11`, `ctx off=136 len=16 8:20 | The exception is 'The assertion in this constant expression failed.' …` (mensagem não-String: código sem mensagem)
  - `off=229 len=11 13:9`, mesmo contexto de `b`

##### `const_eval_property_access` (perda 12: FN 1, FP 11, msg 0, pos 0)
- **Emissão:** `ConstantVisitor._evaluatePropertyAccess` (`evaluation.dart:1686-1727`), de
  `visitPrefixedIdentifier` (`:1123-1128`) e `visitPropertyAccess` (`:1200-1204`).
- **Condição exata:** §2.5.
- **Posição:** o `PrefixedIdentifier`/`PropertyAccess` inteiro (alvo + `.` + nome); dentro de construtor, o nó
  da criação (com contexto).
- **Mensagem:** `The property '{0}' can't be accessed on the type '{1}' in a constant expression.` {0} =
  `identifier.name`; {1} = `targetType.getDisplayString()` — `String` pronta do tipo **do valor** (sem alias):
  `List<int>`, `(int, int)`, `A`, `Duration`. Quando o erro vem da direita de `&&`/`||` os argumentos se perdem
  e a mensagem sai com `{0}`/`{1}` literais (§2.3).
- **Supressões e ordem:** alvo que não avalia → o erro do alvo; propriedade de extensão →
  `const_eval_extension_method`; `length` de `String` → valor. Convive com `undefined_getter`.
- **No DartForge:** `avaliador.rs:1283-1336` (`propriedade`, `acesso_a_propriedade`), fiel na condição. FP (11):
  `const_with_type_parameters/ConstWithTypeParametersConstructorTearo_{283eab6c,4c51d08d,990aed62,995748f2,f7549011}`
  (`A<T>.new`) e `patterns/invalid_const_pattern_test.dart:127,291,543,550,579,583` (`prefix.Class.named`,
  `GenericClass<int>.new`, `prefix.GenericClass<int>.new`): `propriedade` só trata o alvo como "interface" se
  ele é identificador **simples** que resolve para classe (`:1286`, `:1305`); com alvo `prefix.Classe` ou
  `Classe<int>` avalia o alvo (um `Type`) e cai em `CONST_EVAL_PROPERTY_ACCESS` com `'Type'`, sem olhar que o nó
  resolve para `Resolved::Constructor`. No oficial esses nós são `ConstructorReference` (valor função, §2.2) —
  nenhum erro de avaliação (`v01`; o único diagnóstico é `invalid_constant_pattern_generic` do parser em
  `case GenericClass<int>.new`). FN (1) `primary_constructors/const/potentially_constant_error_test.dart:57:17`:
  `d.length` com `d` dynamic valendo lista, em construtor primário (oráculo 3.13; não rodado aqui) — o
  equivalente 3.6.2 (`s26`) confirma a regra: o tipo do **valor** decide. Mudança: em `propriedade`, nó com
  `Resolved::Constructor` → `valor_constante` direto (antes de avaliar o alvo).
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  const a = 'abc'.length;
  const b = [1].length;
  const s = 'x';
  const c = s.length;
  const d = s.isEmpty;
  const e = (1, 2).$1;
  const f = 1.isEven;
  ```
  - `const_eval_property_access off=34 len=10 2:11 | The property 'length' can't be accessed on the type 'List<int>' in a constant expression.`
  - `off=91 len=9 5:11 | … 'isEmpty' … 'String' …`; `off=112 len=9 6:11 | … '$1' … '(int, int)' …`; `off=133 len=8 7:11 | … 'isEven' … 'int' …`
  ```dart
  class A {
    final int a;
    const A(dynamic d) : a = d.length;
  }
  const x = const A(<int>[]);
  const y = const A('abc');
  ```
  - `const_eval_property_access off=74 len=16 5:11 | The property 'length' can't be accessed on the type 'List<int>' in a constant expression.`, `ctx off=52 len=8 3:28 | The error is in the field initializer of 'A', and occurs here.` (`y` limpo)
  ```dart
  class A {
    final int f;
    const A(this.f);
  }
  const a = const A(1).f;
  const o = const A(1);
  const b = o.f;
  ```
  - `off=56 len=12 5:11 | The property 'f' can't be accessed on the type 'A' …`; `off=102 len=3 7:11` idem

##### `const_eval_method_invocation` (perda 8: FN 0, FP 8, msg 0, pos 0)
- **Emissão:** `ConstantVisitor.visitMethodInvocation` (`evaluation.dart:1023-1059`).
- **Condição exata:** nó `MethodInvocation` que não é `identical` de `dart:core` com 2 argumentos e cujo
  `staticType` não é `InvalidType` (aí: `INVALID_CONSTANT` não resolvido). Nada é avaliado antes (nem alvo nem
  argumentos). Criação implícita (`A(1)` sem `new`/`const`) **não** é `MethodInvocation` — o resolvedor a
  reescreve como `InstanceCreationExpression`, inclusive para extension type e para construtor gerador de enum.
- **Posição:** a invocação inteira (alvo, nome, argumentos de tipo, argumentos).
- **Mensagem:** `Methods can't be invoked in constant expressions.`
- **Supressões e ordem:** método de extensão chamado como método também dá este código (`1.m()`), não
  `const_eval_extension_method` (este é só para operador e getter). `identical` com nº errado de argumentos →
  este código + `not_enough_positional_arguments`.
- **No DartForge:** `avaliador.rs:423-464`. FP (8): `Call` sem `Resolved::Constructor` cujo alvo é um
  extension type ou enum — `RelationalPatternArgumentTypeNotAssigna_1f8904e4.dart:2:14` (`A(true)`),
  `dot_shorthands/language_defined/bool_context_error_test.dart:12:30,13:31` (`Bool(true)`),
  `dot_shorthands/private/private_extension_type_lib.dart:23:30`,
  `extension_type/relational_pattern_error_test.dart:26:12` (`E(0)`),
  `primary_constructors/const/potentially_constant_error_test.dart:20:17`,
  `enum/enhanced_enums_error_test.dart:517:40,535:16` (`NoConstructorCalls(3)`). A inferência não grava o
  construtor (primário de extension type; gerador de enum com erro
  `invalid_reference_to_generative_enum_constructor`) e o avaliador cai em `:463`. Oficial (`v02`, `s18`):
  `const E0 = E(0)` com `extension type const E(int it)` → sem erro (valor = a representação, §3.3 F);
  `static const N e3 = N(3)` em enum → só `invalid_reference_to_generative_enum_constructor off=57 len=1`.
  Mudança: resolver o construtor nesses dois casos (dono: inferência); no avaliador, `Call` cujo alvo resolve
  para classe/extension type/enum é criação.
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  const a = identical(1, 1);
  const b = identical(1, 1.0);
  const c = identical(const [1], const [1]);
  int f(int x) => x;
  const d = f(1);
  const e = 'a'.toString();
  const g = identical(1);
  extension E on int { int get twice => this * 2; int m() => 0; int operator +(String s) => 0; }
  const h = 1.twice;
  const i = 1.m();
  ```
  - `const_eval_method_invocation off=128 len=4 5:11`; `off=144 len=14 6:11`; `off=170 len=12 7:11` (+ `not_enough_positional_arguments off=181 len=1`); `off=308 len=5 10:11`
  - `const_eval_extension_method off=289 len=7 9:11` (getter de extensão)
  - `a`, `b` (int × double: desconhecido), `c`: limpos

##### `const_eval_type_bool` (perda 6: FN 0, FP 6, msg 0, pos 0)
- **Emissão:** `visitConditionalExpression` (`evaluation.dart:764-767`) e `assertBool` (`value.dart:1595-1599`)
  via `logicalNot`, `lazyAnd`, `lazyOr`, `logicalAnd/Or/Xor`.
- **Condição exata:** condição de `?:` cujo valor não é `BoolState`; operando de `!`, `&&`, `||` não bool (§2.3).
  (`&`/`|`/`^` com mistura dão `_BOOL_INT`; condição de `if` em coleção dá `NON_BOOL_CONDITION`.)
- **Posição:** condicional: **a condição**; `!x`: a expressão prefixa inteira; `&&`/`||`: a binária inteira,
  qualquer que seja o lado errado.
- **Mensagem:** `In constant expressions, operands of this operator must be of type 'bool'.`
- **Supressões e ordem:** condição `null` também cai aqui (não em "throws"); `false && <não bool>` não avalia a
  direita (sem erro se ela é potencialmente constante).
- **No DartForge:** `avaliador.rs:483-515`, `:972-1005`, `valor.rs:119-121`. FP (6)
  `string/multiline_newline_test.dart:79…109`: a condição `cr.s == crlf.s ? … : null` usada adiante; causa
  conforme §D (normalização de CR/CRLF em literal multilinha), não reconferida nesta rodada.
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  const b = 1 ? 1 : 2;
  const c = null ? 1 : 2;
  ```
  - `const_eval_type_bool off=10 len=1 1:11` e `off=31 len=4 2:11` (a condição; + `non_bool_condition` nos mesmos intervalos)
  ```dart
  const c = !1;
  const d = 1 && true;
  const e = true && 1;
  ```
  - `const_eval_type_bool off=10 len=2 1:11`; `off=24 len=9 2:11`; `off=45 len=9 3:11` (nó inteiro; + `non_bool_negation_expression off=11 len=1`, `non_bool_operand off=24 len=1` e `off=53 len=1`)

##### `const_eval_type_string` (perda 1: FN 1, FP 0, msg 0, pos 0)
- **Emissão:** `assertString` (`value.dart:1628-1632`) em `stringLength` (`:1934-1937`) e `concatenate`
  (`:1681-1684`: parte de `AdjacentStrings`/interpolação que não é `String` depois da conversão — na prática
  inalcançável, pois `performToString` devolve `String`).
- **Condição exata:** `x.length` em que o **tipo** do valor de `x` é `String` mas o **estado** não é
  `StringState` — só acontece com o objeto inválido de `_valueOf` (argumento não resolvido cujo parâmetro é
  `String`).
- **Posição:** o acesso `o.length`; como está em construtor, recolocado no nó da criação com contexto.
- **Mensagem:** `In constant expressions, operands of this operator must be of type 'String'.`
- **Supressões e ordem:** sai junto com o erro relatado por `_valueOf` no argumento (§3.1) e com
  `undefined_identifier`.
- **No DartForge:** as peças existem — `valor_de` (`avaliador.rs:1775-1786`) cria `Null { invalido }` com o
  tipo do parâmetro e `comprimento` (`valor.rs:497-504`) lança `CONST_EVAL_TYPE_STRING`. **Corrige §D
  `const_eval_type_string`** ("`length` vira `CONST_EVAL_PROPERTY_ACCESS`/outro"): a causa do FN
  (`ConstEvalTypeString__length_unresolvedType.dart:8:11`, com FP de
  `const_initialized_with_non_constant_value` em `8:13`) é anterior: o identificador indefinido `x` não está em
  `tipos_invalidos`, então `valor_constante` (`:1410-1413`) devolve o genérico **sem** `nao_resolvida` e a
  criação aborta no argumento. Mesma causa dos FP de `non_constant_default_value` (T4).
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  class B {
    final l;
    const B(String o) : l = o.length;
  }
  const y = B(x);
  ```
  - `const_eval_type_string off=69 len=4 5:11`, `ctx off=47 len=8 3:27 | The error is in the field initializer of 'B', and occurs here.`
  - `const_with_non_constant_argument off=71 len=1 5:13`; `undefined_identifier off=71 len=1 5:13`

##### `const_constructor_throws_exception` (perda 1: FN 1, FP 0, msg 0, pos 0)
- **Emissão:** `ErrorVerifier._checkForConstEvalThrowsException` (`generated/error_verifier.dart:2936-2943`), de
  `visitThrowExpression` (`:1550`). Não é do motor.
- **Condição exata:** `ThrowExpression` com `_enclosingExecutable.isConstConstructor` (`:98`: o executável
  envolvente é construtor `const`) — em inicializador, `assert`, valor padrão ou corpo.
- **Posição:** a `ThrowExpression` inteira (`throw ''`).
- **Mensagem:** `Const constructors can't throw exceptions.`
- **Supressões e ordem:** sai com `invalid_constant` no mesmo intervalo (o `throw` não é potencialmente
  constante, `_reportNotPotentialConstants`). Nas criações, o motor trata o `throw` como genérico: em mensagem de
  `assert` a mensagem "não avalia" e a falha sai como `CONST_EVAL_ASSERTION_FAILURE` sem mensagem.
- **No DartForge:** não existe (nenhuma ocorrência de `CONST_CONSTRUCTOR_THROWS_EXCEPTION` fora de
  `crates/diagnostics/src/codigos_g.rs` e da lista de `verificador.rs`). FN
  `ConstEvalThrowsException__assertInitial_5db8e50b.dart:2:36`. Lugar: verificador de erros (`crates/analise`),
  com o construtor const envolvente.
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  class A {
    const A(int x): assert(x > 0, '${throw ''}');
  }
  const a = const A(0);
  ```
  - `const_constructor_throws_exception off=45 len=8 2:36`; `invalid_constant off=45 len=8 2:36`; `dead_code off=54 len=3 2:45`
  - `const_eval_throws_exception off=70 len=10 4:11`, `ctx off=28 len=28 2:19 | The exception is 'The assertion in this constant expression failed.' and occurs here.`

##### `const_constructor_field_type_mismatch` (perda 8: FN 1, FP 7, msg 0, pos 0)
- **Emissão:** `_checkFields` (`evaluation.dart:2635-2646`) e `_checkInitializers` (`:2731-2752`).
- **Condição exata:** §3.3 fases A e D. O código só **sobrevive** quando não é "de execução": fase A com tipo
  declarado do campo sem parâmetro de tipo; fase D com inicializador estaticamente **não** atribuível ao campo.
  Nos outros casos vira `const_eval_throws_exception` com este texto no contexto.
- **Posição:** fase D: `_errorNode` (a criação / a constante de enum). Fase A: o inicializador do campo **na
  declaração** (`field.constantInitializer`), sem `copyWithEntity` — o erro não é recolocado no uso e, sendo o
  mesmo intervalo para toda criação, sai uma vez só (último exemplo abaixo).
- **Mensagem:** `In a const constructor, a value of type '{0}' can't be assigned to the field '{1}', which has
  type '{2}'.` — {0}, {2} = `getDisplayString()` (String pronta); {2} é o tipo substituído na fase A e
  `field.type` na fase D.
- **Supressões e ordem:** antes dele, `CONST_EVAL_THROWS_EXCEPTION` se o campo já estava no mapa. Convive com
  `field_initializer_not_assignable` (estático).
- **No DartForge:** `avaliador.rs:1920-1946` (fase A) e `:2066-2082` (fase D). FN
  `ConstFieldInitializerNotAssignable__enu_2118db2a.dart:2:3`: constante de enum não avaliada pelo construtor.
  FP: `ConstConstructorFieldTypeMismatchContex_0a10b92c.dart:2:15` — `excecao` calculada sobre o tipo
  substituído (`:1937-1939`), deveria ser sobre o declarado (`T`) → o erro deixa de ser convertido;
  `const/instantiated_function_constant_error_test.dart:196,197,200,210` e
  `instantiated_function_constant_test.dart:79,80` (6): mensagem "`void Function(U, [num, List<U>])` … field
  'x7'" — o valor do tear-off instanciado implicitamente guarda o tipo com `U` livre; o oficial instancia e
  **substitui** (`_instantiateFunctionType`, `evaluation.dart:1949-1957`) — redução `s25` no oráculo: sem erro.
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  enum E {
    v;
    final int x;
    const E() : x = '';
  }
  ```
  - `const_constructor_field_type_mismatch off=11 len=1 2:3 | In a const constructor, a value of type 'String' can't be assigned to the field 'x', which has type 'int'.`; `field_initializer_not_assignable off=47 len=2 4:19`
  ```dart
  class G<T> {
    final T x = y;
    const G();
  }
  const dynamic y = 1;
  const a = const G<String>();
  class H {
    final int x;
    const H(dynamic v) : x = v;
  }
  const b = const H('a');
  class I {
    final int x;
    const I(String v) : x = v;
  }
  const c = const I('a');
  ```
  - `const_eval_throws_exception off=76 len=17 6:11`, `ctx off=27 len=1 2:15 | The exception is 'In a const constructor, a value of type 'int' can't be assigned to the field 'x', which has type 'String'.' and occurs here.`
  - `const_eval_throws_exception off=162 len=12 11:11`, `ctx off=147 len=1 9:28 | The exception is '… 'String' … field 'x', which has type 'int'.' …`
  - `field_initializer_not_assignable off=227 len=1 14:27`; `const_constructor_field_type_mismatch off=242 len=12 16:11` (na criação: não atribuível estaticamente)
  ```dart
  class A {
    final int x = y;
    const A();
  }
  const dynamic y = 'a';
  const a = const A();
  var b = const A();
  ```
  - `const_constructor_field_type_mismatch off=26 len=1 2:17 | In a const constructor, a value of type 'String' can't be assigned to the field 'x', which has type 'int'.` (único, no `y` da declaração)

##### `const_constructor_param_type_mismatch` (perda 4: FN 2, FP 2, msg 0, pos 0)
- **Emissão:** `_checkParameters` (`evaluation.dart:2903-2939`).
- **Condição exata:** §3.3 fase C, itens (1) e (2). Sobrevive como este código quando: o argumento é
  estaticamente **não** atribuível ao parâmetro; ou não há nó de argumento (valor padrão / `null` implícito:
  o nó é `_errorNode`, cujo tipo estático — o da classe criada — não é atribuível; anotação e constante de enum
  nem são `Expression`); ou é o item (2) (campo de `this.x` com tipo diferente do parâmetro).
- **Posição:** o argumento posicional; o `NamedExpression` **inteiro** (rótulo incluso) no nomeado; `_errorNode`
  quando o valor veio do padrão.
- **Mensagem:** `A value of type '{0}' can't be assigned to a parameter of type '{1}' in a const constructor.`
  {0} = tipo do valor; {1} = `parameter.type` substituído (ou o tipo do campo no item 2); Strings prontas.
- **Supressões e ordem:** valor inválido (`isInvalid`) pula a checagem. Parâmetro opcional cujo padrão é
  inválido é ignorado. `super.x` sem padrão herda o padrão do parâmetro do super (§1.5) — `const B({super.a})`
  sobre `A({int a = 0})` não dá erro (`s03`).
- **No DartForge:** `avaliador.rs:1957-2010`. FN (2): `ArgumentTypeNotAssignable__enumConstant.dart:2:5`,
  `FieldInitializingFormalNotAssignable__e_bcd4aae5.dart:2:5` — argumentos de constante de enum (construtor não
  avaliado). FP (2): `ConstConstructorParamTypeMismatch__supe_9b1ee5a1.dart:9:11`, `_c41cede3.dart:9:11` —
  `:1975-1977` dá `null` a todo opcional sem padrão escrito, inclusive `super.a`; deve usar o padrão do parâmetro
  correspondente do construtor super. Posição do nomeado: `:1744` já usa rótulo + valor.
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  class A {
    const A(int x);
  }
  class B<T> {
    final T f;
    const B(this.f);
  }
  const dynamic d = 'a';
  const a = const A(d);
  const b = const A('a');
  const c = const B<int>(d);
  ```
  - `const_eval_throws_exception off=110 len=10 9:11`, `ctx off=118 len=1 9:19 | The exception is 'A value of type 'String' can't be assigned to a parameter of type 'int' in a const constructor.' and occurs here.`
  - `argument_type_not_assignable off=140 len=3 10:19`; `const_constructor_param_type_mismatch off=140 len=3 10:19 | A value of type 'String' can't be assigned to a parameter of type 'int' in a const constructor.`
  - `const_eval_throws_exception off=156 len=15 11:11`, `ctx off=169 len=1 11:24` (mesma mensagem)
  ```dart
  class P { final int q; const P([this.q = 'x' as dynamic]); }
  const p = const P();
  class R { final String q; const R([this.q]); }
  const r = const R();
  ```
  - `const_constructor_param_type_mismatch off=71 len=9 2:11 | A value of type 'String' can't be assigned to a parameter of type 'int' in a const constructor.` e `off=139 len=9 4:11 | A value of type 'Null' … 'String' …` — na criação (+ `missing_default_value_for_parameter off=122 len=1`)
  ```dart
  class A {
    final int x;
    const A({required this.x});
  }
  const a = const A(x: 'a' as dynamic);
  ```
  - `const_eval_throws_exception off=67 len=26 5:11`, `ctx off=75 len=17 5:19` (`x: 'a' as dynamic`, com o rótulo)

##### `recursive_compile_time_constant` (perda 12: FN 12, FP 0, msg 0, pos 0)
- **Emissão:** `generateCycleError` grava o `InvalidConstant` (`evaluation.dart:423-425`); relato por
  `ConstantVerifier.visitVariableDeclaration` / `visitEnumConstantDeclaration` (§1.6).
- **Condição exata:** a variável (const de topo/estática/local, `final` de instância de classe com construtor
  const, constante de enum) pertence a um SCC do grafo de §1.3–1.4 (ou tem laço trivial) — incluindo o falso
  positivo do item 3 de §1.4.
- **Posição:** o nome da variável (`nameOffset`, `nameLength`); constante de enum: só o nome (`v1`), não os
  argumentos.
- **Mensagem:** `The compile-time constant expression depends on itself.`
- **Supressões e ordem:** um por variável do SCC. Quem lê a variável não relata nada (`avoidReporting`): nem o
  uso em outra const (`const z = y`), nem elemento de lista (`bad_initializer1`: sem
  `non_constant_list_element`), nem a criação que a recebe como argumento. `final` de instância em classe sem
  construtor **gerador** const não relata. `values` sintético não relata.
- **No DartForge:** só ciclo variável→variável visto durante a avaliação (`avaliador.rs:1453-1463`,
  `:1517-1520`; `pilha_vars`/`ciclicos`), sem construtores, enum, campos `final` nem locais. FN (12):
  `NonExhaustiveSwitchStatement__alwaysExh_050cebf1.dart:2:3,2:11`, `RecursiveCompileTimeConstant__enum_constants.dart:2:3,2:11`,
  `enum/enhanced_enums_error_test.dart:638,643` (enum `v1(v2), v2(v1)`);
  `RecursiveCompileTimeConstant__enum_cons_df2a651a.dart:2:3`, `enhanced_enums_error_test.dart:653` (`v(values)`);
  `RecursiveConstantConstructor__typeName__491914a2.dart:1:7` (`const y = const C()` × `C() : x = y`);
  `RecursiveConstantConstructor__typeName_field.dart:5:9` (`final m = const A()`); `const/syntax_test.dart:109:16`;
  `variable/bad_initializer1_test.dart:8:9` (local). Mudança: o grafo de §1 no `Motor`, antes do verificador.
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  const a = b + 1;
  const b = c + 1;
  const c = a + 1;
  const d = a;
  ```
  - `recursive_compile_time_constant off=6 len=1 1:7`; `off=23 len=1 2:7`; `off=40 len=1 3:7` (`d` limpo)
  ```dart
  enum E {
    v1(v2), v2(v1);
    const E(Object o);
  }
  ```
  - `off=11 len=2 2:3`; `off=19 len=2 2:11`
  ```dart
  enum CR {
    e1(values);
    final List<CR> list;
    const CR(this.list);
  }
  ```
  - `off=12 len=2 2:3` (único)
  ```dart
  main() {
    const elems = const [
      const [
        1,
        elems,
      ],
    ];
  }
  ```
  - `recursive_compile_time_constant off=17 len=5 2:9`; `referenced_before_declaration off=60 len=5 5:7`
  ```dart
  class A { static const a = B.b; }
  class B { static const b = C.c; }
  class C { static const c = A.a; }
  const x = A.a;
  const y = [x];
  var z = const [x];
  ```
  - `off=23 len=1 1:24`; `off=57 len=1 2:24`; `off=91 len=1 3:24` (`x`, `y`, `z` limpos)

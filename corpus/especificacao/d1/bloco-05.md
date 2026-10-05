
##### `recursive_constant_constructor` (perda 14: FN 14, FP 0, msg 0, pos 0)
- **Emissão:** `ConstantVerifier.visitConstructorDeclaration` (`constant_verifier.dart:165-179`), lendo
  `isCycleFree`, que só `evaluateScc` desliga (`compute.dart:69-71`).
- **Condição exata:** `node.constKeyword != null && element is ConstructorElementImpl && !element.isCycleFree &&
  !element.isFactory`. O construtor está num SCC (ou laço trivial) com as arestas de §1.3: `this(...)`,
  `super(...)`, criação const em inicializador, const lida em inicializador, campo `final` com inicializador
  que cria a própria classe, valor padrão de parâmetro. Factory em ciclo fica `isCycleFree = false` mas não
  relata aqui (`recursive_factory_redirect` é do `ErrorVerifier`).
- **Posição:** `node.returnType` — o identificador da classe no cabeçalho do construtor (len = nome da classe),
  mesmo em construtor nomeado (`C.a` → só o `C`).
- **Mensagem:** `The constant constructor depends on itself.`
- **Supressões e ordem:** um por construtor do SCC. As criações por ele não dão erro (valor desconhecido).
  Convive com `recursive_constructor_redirect` (no `this(...)`/`this.a()` de cada um) e com
  `recursive_compile_time_constant` nas variáveis do mesmo SCC.
- **No DartForge:** não existe (o código não é emitido em lugar nenhum de `crates/`); `avaliador.rs:1717-1719`
  só evita a recursão com `construtores_em_curso` (valor desconhecido) — sem marcar o construtor. FN (14):
  3.6.2: `RecursiveConstantConstructor__typeName__491914a2.dart:5:9`, `__50f1333c.dart:3:9`,
  `__75ac2c77.dart:3:9,9:9`, `_field.dart:2:9`, `const/syntax_test.dart:115:9`; oráculo 3.13 (sintaxe nova, não
  rodados aqui): `AssertInRedirectingConstructor__enum_pr_{bf07cd7e,ea921e48}.dart:3:9`,
  `FieldInitializerRedirectingConstructor__{1a923327,669dd8a8}.dart:4:9`,
  `MultipleRedirectingConstructorInvocatio_86b24eda.dart:3:9,6:9`,
  `RecursiveConstantConstructor__redirectC_{0523ea3a,d6dc6167}.dart:2:9`. Mudança: grafo (§1), campo
  `ciclo_livre` por construtor, relato no nome da classe do cabeçalho.
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  class A {
    final m = const A();
    const A();
  }
  class C {
    final x;
    const C() : x = y;
  }
  const y = const C();
  class R { const R() : this.a(); const R.a() : this(); }
  class S { const S([x = const S()]); }
  class T { const T() : this(); }
  ```
  - `recursive_compile_time_constant off=18 len=1 2:9` (`m`); `recursive_constant_constructor off=41 len=1 3:9`
  - `recursive_constant_constructor off=77 len=1 7:9`; `recursive_compile_time_constant off=98 len=1 9:7` (`y`)
  - `recursive_constant_constructor off=129 len=1 10:17` e `off=151 len=1 10:39`; `recursive_constructor_redirect off=135 len=8 10:23` e `off=159 len=6 10:47`
  - `recursive_constant_constructor off=185 len=1 11:17` (ciclo pelo valor padrão)
  - `recursive_constant_constructor off=223 len=1 12:17`; `recursive_constructor_redirect off=229 len=6 12:23`
  ```dart
  class B {
    final A a;
    const B() : a = const A();
  }
  class A {
    final B b;
    const A() : b = const B();
  }
  class Z {
    final Z z;
    const Z() : z = const Z();
  }
  const k = const A();
  ```
  - `recursive_constant_constructor off=31 len=1 3:9`; `off=85 len=1 7:9`; `off=139 len=1 11:9` (`k` limpo)

##### `invalid_constant` (perda 17: FN 3, FP 6, msg 0, pos 8)
- **Emissão:** como **código final** sai por três vias: (a) verificador, `_reportNotPotentialConstants`
  (`constant_verifier.dart:749-762`) em inicializadores de construtor const (D.R4-2); (b) caminho 2 de §5 — o
  `InvalidConstant` cru de uma criação const: genérico de argumento **nomeado** (`value.dart:2483-2485`),
  construtor/tear-off não resolvido (`evaluation.dart:806-808`, `:829-831`, `:934-935`), invocação com tipo
  inválido (`:1052-1054`), argumento de tipo que não é tipo (`:899-901`), nó não potencialmente constante num
  ramo morto (`:1999-2000`); (c) `_valueOf` relatando argumento não resolvido (`:2011-2019`). Como código
  **interno** é o "genérico" trocado pelo padrão do ponto de uso (§5).
- **Condição exata:** §2.1 (`genericError`), §2.2, §2.4.
- **Posição:** o nó genérico. Identificador sintético (recuperação do parser) tem **length 0**.
- **Mensagem:** `Invalid constant value.`
- **Supressões e ordem:** em variável `const` aparece ao lado do padrão no mesmo intervalo (§5, exemplo). Num
  inicializador de construtor const, `_reportNotPotentialConstants` relata **todos** os nós ruins; o motor, só o
  primeiro.
- **No DartForge:** `avaliador.rs:270-275` (`generico`: `args_de_criacao` faz o papel do teste de pai) e
  `potencial.rs`. FN (3): `const/constant_type_variable_error_test.dart:32:19` (`const <X, String?>{}` em
  inicializador: argumentos de tipo de **mapa** têm de ser tipos constantes, `potentially_constant.dart:370-378`;
  de lista/conjunto, potencialmente constantes, `:346-352`, `:362-367` — `potencial.rs` não olha tipos);
  `string/interpolation1_test.dart:14:35` (`const A("$")`: identificador sintético na interpolação, não
  resolvido → via (c), len 0); `primary_constructors/const/potentially_constant_error_test.dart:45:17` (3.13).
  FP (6) `dot_shorthands/equality/equality_ctor_error_test.dart:11…46:14` e pos (8)
  `dot_shorthands/equality/equality_error_test.dart` — atalhos de ponto, oráculo 3.13.4, não rodados aqui;
  causa conforme §D.
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  class K<X> {
    final x6;
    const K() : x6 = const <X, String?>{};
  }
  ```
  - `invalid_constant off=51 len=1 3:27`; `invalid_type_argument_in_const_literal off=51 len=1 3:27 | Constant map literals can't use a type parameter in a type argument, such as 'X'.`
  ```dart
  class A {
    final String str;
    const A(this.str);
  }
  class S {
    static const DOLLAR = const A("$");
  }
  ```
  - `invalid_constant off=97 len=0 6:35`; `missing_identifier off=97 len=1 6:35` (sem `const_initialized_with_non_constant_value`: a criação segue com objeto inválido)
  ```dart
  class A {
    final int x;
    const A(int y) : x = y.foo;
  }
  class B extends A {
    const B(int y) : super(y);
  }
  const a = const A(1);
  const b = const B(1);
  ```
  - `invalid_constant off=48 len=5 3:24` (via (a), na declaração) + `undefined_getter off=50 len=3`
  - `const_eval_property_access off=118 len=10 8:11 | The property 'foo' can't be accessed on the type 'int' …`, `ctx off=48 len=5 3:24 | The error is in the field initializer of 'A', and occurs here.`
  - `const_eval_property_access off=140 len=10 9:11`, `ctx off=48 len=5 3:24` (idem) e `ctx off=85 len=1 6:9 | The evaluated constructor 'A' is called by 'B' and 'B' is defined here.`

##### `const_initialized_with_non_constant_value` (perda 20: FN 6, FP 6, msg 0, pos 8)
- **Emissão:** `ConstantVerifier.visitVariableDeclaration` (`constant_verifier.dart:503-506`): código **padrão**
  de `_reportError` para o `evaluationResult` de variável `const`.
- **Condição exata:** variável `const` com inicializador cujo resultado guardado é `InvalidConstant` com
  `!avoidReporting` e código fora dos ESPECÍFICOS (§5): `INVALID_CONSTANT`, `CONST_WITH_NON_CONSTANT_ARGUMENT`,
  `CONST_WITH_NON_CONST`, `MISSING_CONST_IN_*`, `MAP_ENTRY_NOT_IN_MAP`, `CONST_EVAL_TYPE_TYPE`,
  `CONST_CONSTRUCTOR_CONSTANT_FROM_DEFERRED_LIBRARY`.
- **Posição:** `error.offset`/`length` — o **subnó** que falhou (o identificador não const, o operando de
  `++v`, o ramo morto não potencialmente constante, a palavra `const` de criação não-const…), não o
  inicializador inteiro; exceção: o marcador de expressão não serializável (§1.7), que ocupa o inicializador
  inteiro em variável não local.
- **Mensagem:** `Const variables must be initialized with a constant value.` (sem argumentos, sem contextos —
  o padrão descarta os do erro).
- **Supressões e ordem:** nada se a leitura foi de const inválida (`avoidReporting`), nem em literal `{…}`
  ambíguo, nem se a variável é `final` (padrão `null`). Coexiste, no mesmo intervalo, com o código cru do
  caminho 2 e com o padrão do literal (`non_constant_list_element`…).
- **No DartForge:** `verificador.rs:178-188`, `:209-215`, `relatar_invalida` `:151-160`. FN (6):
  `const_eval_for_element/ConstEvalForElement__{listLiteral,listLiteral_forIn,mapLiteral_forIn,mapLiteral_forIn_nested,setLiteral_forIn}.dart`
  (5; falta o marcador de §1.7: o motor dá `CONST_EVAL_FOR_ELEMENT`, específico, e o padrão não sai);
  `method/not_found_test.dart:11:24` (`const B()` com `B` método: oficial → `INVALID_CONSTANT` no nó da criação
  → padrão, len 9; causa do nosso silêncio **não verificada**). FP (6): `ConstEvalThrowsException__fromEnvironme_4dc71b35.dart:1:11`
  (patch do SDK, §D); `ConstEvalTypeString__length_unresolvedType.dart:8:13` (ver `const_eval_type_string`);
  `ConstInitializedWithNonConstantValueFro_d5ca985c.dart:7:11` (falta o ramo deferred, §2.6);
  `ConstWithNonConst__mixinApplication_con_b74b8b3c.dart:6:11` e `ExperimentalMemberUse__incorrectlyNeste_0e44e3bf.dart:16:11`
  (§D `const_with_non_const`); `variable/bad_initializer1_test.dart:17:7` (leitura de const em ciclo deve ser
  `evitar_relato`). pos (8) `dot_shorthands/equality/equality_error_test.dart` (3.13, §D). Além disso
  `relatar_invalida` descarta erro cuja unidade difere da atual (`verificador.rs:152`); o oficial relata no
  arquivo corrente com o offset do erro (§5, não verificado ao vivo).
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  var v = 1;
  const a = v++;
  const b = ++v;
  const c = -v;
  const d = v = 2;
  const e = () {};
  const f = [() {}];
  const h = v!;
  const j = (v);
  const k = v..toString();
  ```
  - `const_initialized_with_non_constant_value` em `off=21 len=3 2:11`; `off=38 len=1 3:13`; `off=52 len=1 4:12`; `off=65 len=5 5:11`; `off=82 len=5 6:11`; `off=99 len=7 7:11` (+ `non_constant_list_element off=100 len=5 7:12`); `off=118 len=2 8:11`; `off=133 len=1 9:12`; `off=147 len=13 10:11`
  ```dart
  const a = true ? 1 : 'x'.foo;
  const b = 1 ? 1 : 2;
  const c = null ? 1 : 2;
  const d = null ?? 1;
  const e = 1 ?? v;
  const f = false && v;
  const g = true || v;
  var v = true;
  const h = v ? 1 : 2;
  const i = true ? v : 2;
  ```
  - `off=21 len=7 1:22` (ramo morto `'x'.foo`: não potencialmente constante); `off=111 len=1 5:16`; `off=133 len=1 6:20`; `off=154 len=1 7:19`; `off=181 len=1 9:11`; `off=209 len=1 10:18` (mais `dead_code`, `undefined_getter`, `dead_null_aware_expression`, e os `const_eval_type_bool` de `b`/`c`)
  ```dart
  class A {
    B();
    static const field = const B();
  }
  ```
  - `const_initialized_with_non_constant_value off=40 len=9 3:24`; `creation_with_non_type off=46 len=1 3:30`; `concrete_class_with_abstract_member off=12 len=4 2:3`
  ```dart
  var x = true;
  var l = [1];
  const a = [if (x) 1];
  const b = [...l];
  const c = [for (var i = 0; i < 1; i++) i];
  ```
  - `off=42 len=1 3:16` + `non_constant_list_element off=42 len=1`; `off=63 len=1 4:15` + `non_constant_list_element off=63 len=1`; `off=77 len=31 5:11` + `const_eval_for_element off=78 len=29 5:12`

##### `const_initialized_with_non_constant_value_from_deferred_library` (perda 1: FN 1, FP 0, msg 0, pos 0)
- **Emissão:** `_getDeferredLibraryError` (`evaluation.dart:1878-1934`); específico em `_reportError`.
- **Condição exata:** §2.6 com ancestral `ConstantContextForExpressionImpl` (inicializador de variável não
  local, avaliado sobre a cópia do elemento) ou `VariableDeclaration` (local) **antes** de qualquer outro da
  tabela — dentro de lista, mapa, criação etc. sai o código daquele ancestral.
- **Posição:** o identificador logo depois do prefixo deferred (`pi` em `m.pi`; `E` em `self.E.f`).
- **Mensagem:** `Constant values from a deferred library can't be used to initialize a 'const' variable.`
- **Supressões e ordem:** substitui o padrão (um diagnóstico só). `const self.E()` dá só
  `const_deferred_class` (ErrorVerifier).
- **No DartForge:** não existe ramo deferred no avaliador nem em `potencial.rs` (o oficial também trata
  identificador deferred como não potencialmente constante, `potentially_constant.dart:217-221`, `:301-305`). FN
  `ConstInitializedWithNonConstantValueFro_d5ca985c.dart:7:16` (com o FP correspondente do código padrão em
  `7:11`). Lugar: `avaliador.rs::propriedade` (`:1288-1293`, ramos de prefixo e de extensão), consultando
  `Import.deferred`, com a tabela de ancestrais de §2.6.
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  import 's21.dart' deferred as self;
  class E { static const f = 1; const E(); }
  const a = self.E.f;
  const b = const self.E();
  const c = self.a;
  ```
  - `const_initialized_with_non_constant_value_from_deferred_library off=94 len=1 3:16`; `off=140 len=1 5:16`; `const_deferred_class off=115 len=6 4:17`
  ```dart
  import 'dart:math' deferred as m;
  const a = m.pi;
  void f([x = m.pi]) {}
  const b = [m.pi];
  const c = m.Random;
  ```
  - `…_from_deferred_library off=46 len=2 2:13`; `non_constant_default_value_from_deferred_library off=64 len=2 3:15`; `collection_element_from_deferred_library off=85 len=2 4:14`; `const_initialized_with_non_constant_value_from_deferred_library off=102 len=6 5:13`

##### `non_constant_default_value` (perda 3: FN 0, FP 3, msg 0, pos 0)
- **Emissão:** `ConstantVerifier._validateDefaultValues` (`constant_verifier.dart:811-836`), de
  `visitConstructorDeclaration` (`:190`), `visitFunctionExpression` (`:222`), `visitMethodDeclaration` (`:363`):
  padrão de `_evaluateAndReportError(defaultValue, NON_CONSTANT_DEFAULT_VALUE)`.
- **Condição exata:** `DefaultFormalParameter` com valor; `defaultValue.staticType is InvalidType` → **não
  avalia**; senão avalia a expressão da **AST** (não a cópia do elemento) e relata conforme §5. O resultado
  substitui `evaluationResult` do parâmetro.
- **Posição:** o subnó do erro (como em `const_initialized…`).
- **Mensagem:** `The default value of an optional parameter must be constant.`
- **Supressões e ordem:** códigos específicos saem no lugar (`g()` → `const_eval_method_invocation`).
- **No DartForge:** `verificador.rs:384-396`. FP (3): `NonConstantDefaultValue__function_named_e00ea75a.dart:1:17`,
  `__function_posit_5c2277bf.dart:1:17` (`{int x = X}` com `X` indefinido: `tipos_invalidos` não contém o
  identificador indefinido, `:388`; o oficial tem `InvalidType` e pula — T4);
  `MissingDefaultValueForParameter__constr_521ba5b8.dart:4:12` (`{E a = const E(0)}` com extension type: criação
  não resolvida; ver `const_eval_method_invocation`).
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  int g() => 1;
  var v = 1;
  void f([int a = v, int b = g(), c = undefinedName, d = const [v]]) {}
  class A {
    const A([this.x = v]);
    final int x;
  }
  ```
  - `non_constant_default_value off=41 len=1 3:17`; `const_eval_method_invocation off=52 len=3 3:28`; `undefined_identifier off=61 len=13 3:37` (só); `non_constant_default_value off=87 len=1 3:63` + `non_constant_list_element off=87 len=1`; `non_constant_default_value off=125 len=1 5:21`
  ```dart
  void f({int x = X}) {}
  void g([int x = X]) {}
  class E { const E(int i); }
  extension type ET(int i) { const ET.c(this.i); }
  class A { const A({ET a = const ET.c(0)}); }
  ```
  - só `undefined_identifier off=16 len=1 1:17` e `off=39 len=1 2:17`

##### `non_constant_list_element` (perda 1: FN 0, FP 1, msg 0, pos 0)
- **Emissão:** `ConstantVerifier.visitListLiteral` (`constant_verifier.dart:294-308`) → `_ConstLiteralVerifier.verify`
  (`:1079-1163`): código padrão de `_evaluateAndReportError` para cada elemento-expressão, condição de `if` e
  expressão de spread de lista `const`.
- **Condição exata:** lista com `isConst`; por elemento, o motor avalia a **sub-raiz** e o padrão sai se o erro
  não é específico. Ramo de `if` não tomado (ou os dois, com condição desconhecida) só é checado por
  `_reportNotPotentialConstants`.
- **Posição:** o subnó do erro dentro do elemento.
- **Mensagem:** `The values in a const list literal must be constants.`
- **Supressões e ordem:** leitura de const inválida/em ciclo → nada (`avoidReporting`). Um por elemento ruim
  (todos, não só o primeiro). Em variável const, o primeiro repete-se como
  `const_initialized_with_non_constant_value`.
- **No DartForge:** `verificador.rs:896-1033` (`verificar_elemento`). FP `variable/bad_initializer1_test.dart:17:7`:
  `elems` lido dentro do próprio inicializador local — `valor_de_local` (`avaliador.rs:1575-1585`) devolve o
  genérico não resolvido **sem** `evitar_relato` quando a local está em cálculo (`None`); o oficial já tem o
  `InvalidConstant` de ciclo guardado e devolve `avoidReporting`. Resolve com o grafo cobrindo locais.
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  class A { final int x; const A(this.x); }
  const List<A> l = [A(1 ~/ 0)];
  const m = [1, 'a'.foo, v];
  var v = 1;
  const n = <int>[null as dynamic];
  ```
  - `const_eval_throws_exception off=61 len=9 2:20`, `ctx off=63 len=6 2:22 | The exception is 'Evaluation of this constant expression throws an IntegerDivisionByZeroException.' and occurs here.`
  - `const_eval_property_access off=87 len=7 3:15` (+ `undefined_getter off=91 len=3`); `non_constant_list_element off=96 len=1 3:24` (o `v`: segundo elemento ruim)
  - `list_element_type_not_assignable off=127 len=15 5:17`

##### `non_constant_case_expression` (perda 2: FN 2, FP 0, msg 0, pos 0)
- **Emissão:** `ConstantVerifier._validateSwitchStatement_nullSafety` (`constant_verifier.dart:1009-1046`), de
  `visitSwitchStatement` (`:470`) quando a biblioteca **não** tem `patterns` (< 3.0): padrão de
  `_evaluateAndReportError(expressão do case, NON_CONSTANT_CASE_EXPRESSION)`; para `SwitchPatternCase` sem a
  feature, sobre `pattern.expression.unParenthesized` de um `ConstantPattern` (`:1039-1042`).
- **Condição exata:** erro não específico na avaliação da expressão do `case` (§5).
- **Posição:** o subnó do erro: para `case ERROR_B = 1:` a `AssignmentExpression` inteira (genérico de
  `visitNode`).
- **Mensagem:** `Case expressions must be constant.`
- **Supressões e ordem:** específicos no lugar (`case 1 ~/ 0` → `const_eval_throws_idbze`). Convive com
  `assignment_to_const`.
- **No DartForge:** `verificador.rs:482-488` (ramo `!padroes_ligados`, só para `PatternKind::Constant`). FN
  `switch/case_expression_with_assignment_test.dart:18:10` e `…_legacy_test.dart:18:10` (os dois `// @dart=2.19`):
  causa **não verificada** (candidata de §D: o parser não produz padrão constante com atribuição nesse `case`).
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  // @dart=2.19
  const ERROR_A = 0;
  const ERROR_B = 1;
  errorToString(error) {
    switch (error) {
      case ERROR_A:
        return "ERROR_A";
      case ERROR_B = 1:
        return "x";
    }
  }
  ```
  - `assignment_to_const off=145 len=7 8:10`; `non_constant_case_expression off=145 len=11 8:10`
  ```dart
  // @dart=2.19
  var v = 1;
  const k = 1;
  void f(int x) {
    switch (x) {
      case v:
        break;
      case k + 1:
        break;
      case 1 ~/ 0:
        break;
    }
  }
  ```
  - `non_constant_case_expression off=78 len=1 6:10`; `const_eval_throws_idbze off=132 len=6 10:10`

  Sem o comentário de versão (3.6), `case ERROR_B = 1:` é erro de sintaxe (`expected_token off=133 len=1`,
  `missing_identifier`, `unexpected_token`…) e este código não sai (`u02`).

##### `const_with_non_constant_argument` (perda 19: FN 19, FP 0, msg 0, pos 0)
- **Emissão:** (a) motor: `InvalidConstant.genericError` (`value.dart:2476-2482`) para argumento **posicional
  direto** de `InstanceCreationExpression` const, relatado cru pelo caminho 2 de §5 ou por `_valueOf`;
  (b) verificador: `_validateConstantArguments` (`constant_verifier.dart:779-786`) como código **padrão**, para
  cada argumento (sem o rótulo) de anotação com construtor const (`visitAnnotation`, `:103-127`) e de constante
  de enum (`visitEnumConstantDeclaration`, `:207-210`).
- **Condição exata:** (a) o nó genérico é filho direto da `ArgumentList` da criação; (b) erro não específico
  ao avaliar o argumento isolado — inclusive `isUnresolved` (identificador indefinido em anotação).
- **Posição:** (a) o argumento; (b) o subnó do erro dentro do argumento.
- **Mensagem:** `Arguments of a constant creation must be constant expressions.`
- **Supressões e ordem:** argumento **nomeado** de criação → `invalid_constant` (§2.1); em anotação/enum o
  nomeado dá este código (o padrão não distingue). Em (b) erros específicos saem com o código deles no
  argumento (`@Ann(1 ~/ 0)` → `const_eval_throws_idbze` no `1 ~/ 0`, não "throws exception" na anotação).
  Anotação com construtor não const → só `non_constant_annotation_constructor`.
- **No DartForge:** (a) existe (`avaliador.rs:270-275`, `:1692-1696`, relatos de `valor_de` `:1777-1780`).
  (b) não existe: `verificador.rs` não visita anotações (nenhuma ocorrência de anotação/metadados no arquivo) e,
  nas constantes de enum, só desce nos argumentos com `self.expr` (`:193-200`) sem avaliar com padrão. FN (19):
  `ConstWithNonConstantArgument__annotation.dart:5:4`, `__classShad_cfefb11d.dart:8:15`,
  `undefined_identifier/UndefinedIdentifier__annotation_referen_{1bafa0f6,1c797d93,2549bcdf,5e97ca6d,8bfdb1d4,de31e691}.dart`,
  `__annotation_uses_sc_{3b22b2d4,aef6b030,fee8f445}.dart`, `metadata/type_parameter_scope_inner_test.dart:12…62`
  (7) — anotações; `ConstWithNonConstantArgument__enumConstant.dart:4:5` — enum. Mudança: visitar todas as
  anotações (declarações, membros, parâmetros, parâmetros de tipo, diretivas) e os argumentos de enum com
  `avaliar_e_relatar(arg, true, CONST_WITH_NON_CONSTANT_ARGUMENT)`; pré-requisito: a inferência resolver as
  expressões dos argumentos de anotação (**não verificado** se já resolve).
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  class A { const A(Object o); }
  @A(v)
  class C {}
  var v = 0;
  enum E { v(a); const E(Object o); }
  var a = 1;
  @A(foo)
  class D<@A(foo) T> {}
  ```
  - `const_with_non_constant_argument off=34 len=1 2:4`; `off=70 len=1 5:12`; `off=109 len=3 7:4` (+ `undefined_identifier`); `off=125 len=3 8:12` (+ `undefined_identifier`)
  ```dart
  class Annotation {
    const Annotation(Object obj);
  }
  class Bar {}
  class Foo {
    @Annotation(Bar)
    set Bar(int value) {}
  }
  ```
  - `const_with_non_constant_argument off=92 len=3 6:15`; `undefined_identifier off=92 len=3 6:15`
  ```dart
  class A { const A(); }
  @A.x()
  class B {}
  var v = 1;
  class Ann { const Ann(Object o); Ann.nc(); }
  @Ann(v)
  class C {}
  @Ann.nc()
  class D {}
  @Ann(1 ~/ 0)
  class E {}
  @v
  class F {}
  ```
  - `invalid_annotation off=23 len=6 2:1`; `const_with_non_constant_argument off=102 len=1 6:6`; `non_constant_annotation_constructor off=116 len=9 8:1`; `const_eval_throws_idbze off=142 len=6 10:6`; `invalid_annotation off=161 len=2 12:1`

#### 7. Outros exemplos do oráculo vivo (comportamentos de §2–§4 não cobertos acima)

```dart
const a = 1 ~/ 0;
const b = 1 % 0;
const c = 1 / 0;
const d = 1.5 ~/ 0;
const e = 1 << -1;
const f = 1 >>> -1;
```
- `const_eval_throws_idbze off=10 len=6 1:11 | Evaluation of this constant expression throws an IntegerDivisionByZeroException.`
- `const_eval_throws_exception off=28 len=5 2:11`; `off=62 len=8 4:11`; `off=82 len=7 5:11`; `off=101 len=8 6:11` (`c` limpo)

```dart
const a = 'a' + 1;
const b = 1 + 'a';
const c = !1;
const d = 1 && true;
const e = true && 1;
const f = null + 1;
const g = -'a';
const h = ~1.5;
const i = true & 1;
const j = 1 < 'a';
const k = [] + [];
const l = true + 1;
```
- `const_eval_throws_exception off=10 len=7 1:11` (`'a' + 1`) e `off=104 len=8 6:11` (`null + 1`)
- `const_eval_type_num off=29 len=7 2:11`, `off=124 len=4 7:11`, `off=176 len=7 10:11`; `const_eval_type_int off=140 len=4 8:11`; `const_eval_type_bool_int off=156 len=8 9:11`; `const_eval_type_num_string off=195 len=7 11:11`, `off=214 len=8 12:11`; `const_eval_type_bool` em `c`, `d`, `e` (48/2, 62/9, 83/9)
- mais os estáticos: `argument_type_not_assignable`, `non_bool_negation_expression`, `non_bool_operand`, `invalid_use_of_null_value`, `undefined_operator`

```dart
const u = bool.fromEnvironment('dart.library.js_util');
const l = [if (u) 1];
const c = u ? 1 : 'a'.foo;
const d = u ? 1 : v;
var v = 0;
const e = u && v2;
var v2 = true;
const f = !u;
const g = u ? 1 ~/ 0 : 2;
```
- condição desconhecida: `l` e `f` limpos; os **dois** ramos do condicional contam: `const_eval_property_access off=96 len=7 3:19`; `const_initialized_with_non_constant_value off=123 len=1 4:19`; `off=147 len=7 6:11` (o `&&` inteiro); `const_eval_throws_idbze off=199 len=6 9:15`

```dart
class O { const O(); }
const a = '${const O()}';
const b = '${[1]}';
const c = 'a' 'b' '${1}';
const d = const O() == const O();
const e = 1.0 == 1;
const f = [1] == [1];
class Q { const Q(); bool operator ==(Object o) => true; int get hashCode => 0; }
const g = const Q() == const Q();
const h = const Q() != null;
```
- `const_eval_type_bool_num_string off=34 len=12 2:12` (a `${…}` inteira); `const_initialized_with_non_constant_value off=62 len=3 3:14` (lista fora de contexto constante); `const_eval_type_bool_num_string off=263 len=22 9:11`; `h` limpo (comparação com `null` sempre permitida)

```dart
T id<T>(T x) => x;
const int Function(int) a = id;
const b = id<int>;
class C<T> {
  const C();
  void m<U>() {
    const c = C<T>();
    const d = id<U>;
    const T Function(T) e = id;
    const f = T;
    const g = <T>[];
  }
}
const h = id<int, int>;
```
- `const_with_type_parameters off=128 len=1 7:17` (criação); `off=151 len=1 8:18` (tear-off, no `U`); `off=183 len=2 9:29` (instanciação implícita: no `id`); `const_type_parameter off=201 len=1 10:15`; `invalid_type_argument_in_const_literal off=219 len=1 11:16`; `wrong_number_of_type_arguments_function off=243 len=10 14:13 | The function 'id' is declared with 1 type parameters, but 2 type arguments were given.` (a lista `<int, int>`)

```dart
class A {
  final int x;
  const A(int v) : this.n(v ~/ 0);
  const A.n(this.x) : assert(x > 0);
  const factory A.f(int v) = A.n;
  const A.m(int v) : this.n(v);
}
const a = const A(1);
const b = const A.f(0);
const c = const A.m(0);
```
- `const_eval_throws_exception off=175 len=10 8:11`, `ctx off=51 len=6 3:27 | The exception is '… IntegerDivisionByZeroException.' …` (argumento do `this(...)`)
- `off=197 len=12 9:11` e `off=221 len=12 10:11`, `ctx off=82 len=13 4:23 | The exception is 'The assertion in this constant expression failed.' …` (factory redirecionada e `this.n` chegam ao mesmo `assert`)

```dart
extension type E(int i) { const E.c(this.i); int get twice => i * 2; }
const a = E.c(1);
const b = a.twice;
const c = a + 1;
const d = const E.c('x' as dynamic);
```
- `const_eval_extension_type_method off=99 len=7 3:11`; `undefined_operator off=120 len=1 4:13` (sem erro de constante: o valor é o `int`); `const_eval_throws_exception off=135 len=25 5:11`, `ctx off=145 len=14 5:21`

```dart
const int a = 'x' as dynamic;
const String b = 1 as dynamic;
const List<int> c = <num>[1] as dynamic;
```
- `variable_type_mismatch off=14 len=14 1:15 | A value of type 'String' can't be assigned to a const variable of type 'int'.`; `off=47 len=12 2:18 | … 'int' … 'String'.`; `off=81 len=19 3:21 | … 'List<num>' … 'List<int>'.` (no inicializador inteiro; específico, substitui o padrão)

#### 8. No DartForge: `avaliador.rs` / `valor.rs` hoje × oficial

| tema | oficial | DartForge (disco) | diferença / mudança |
|---|---|---|---|
| quando avalia | tudo antes dos verificadores, por grafo (§1.1–1.4) | sob demanda, dentro de `verificar` (`mod.rs:30-46`), memoizado em `Motor.vars`/`locais`/`padroes` (`avaliador.rs:92-99`) | sem grafo: construir alvos + dependências + Tarjan literal (índices persistentes) e avaliar na ordem, antes do verificador |
| ciclo de variáveis | SCC inteiro, erro no nome, leitura → `avoidReporting` | pilha de avaliação (`:1453-1463`, `:1517-1520`): só ciclos variável→variável de topo/estática | faltam enum, `final` de instância, locais, via construtor/padrão; falso positivo de §1.4-3 a reproduzir |
| ciclo de construtor | `isCycleFree = false` + `recursive_constant_constructor` | `construtores_em_curso` (`:1717-1719`, limite 64) → desconhecido, sem relato | marcar e relatar |
| limite de profundidade | não há | `profundidade > 400` → genérico não resolvido (`:288-290`) | manter só como proteção; com o grafo não deve disparar |
| constante de enum | avalia o construtor (`errorNode` = a declaração), depois `index`/`_name` | `valor_de_enum` (`:1540-1561`): só `index`/`_name` | avaliar o construtor; origem de 7+1+2+1 FN |
| `InvalidConstant` | offset, length, código, args, **contextos**, 3 marcas | `Invalida` (`:29-37`): sem `contextMessages` | acrescentar contextos (saem no JSON do oráculo e no LSP) |
| genérico de argumento | teste de pai na AST (`value.dart:2472-2486`) | conjunto `args_de_criacao` (`:104`, `:271`, `:1692-1696`), salvo/restaurado nas fronteiras | equivalente |
| prefixo `++`/`--` | avalia o operando antes; genérico no nó só se o operando avalia | `unario` (`:941-943`) devolve genérico no nó **antes** de avaliar | inverter a ordem (pos de `constant_pattern_with_non_constant_expression`, §D) |
| direita de `&&`/`\|\|` inválida | reancora no nó binário com o código, sem args (§2.3) | `:986-989`, `:1003`: `Excecao::nova(i.codigo)` @ `e` — igual; args vazios | conferir que a mensagem sai com `{0}`/`{1}` literais quando faltam argumentos |
| condicional | `isBool`, `applyBooleanConversion`, ramos (§2.2) | `:483-515` | igual |
| `as` | substitui o tipo (`_substitution`/ambiente léxico) e só então testa parâmetro de tipo | `:526-540`: tipo estático cru; desiste se menciona parâmetro; não checa `CONST_EVAL_TYPE_TYPE` | aplicar `cx.tipos` antes; 9 FN |
| `is` | `isSubtypeOf(tipo do valor, tipo)` | `:516-525`: sempre bool desconhecido | implementar (afeta `assert(x is T)` e ramos) |
| literal de tipo com argumentos / `FunctionReference` | avalia cada argumento; `typeInstantiate`; erros de §2.2 | `TypeArguments` (`:466-480`): `Tipo(None)` ou função sem argumentos | avaliar argumentos (`CONST_TYPE_PARAMETER` → `…_FUNCTION_TEAROFF`), nº errado, substituição no tipo instanciado (6 FP de `field_type_mismatch`) |
| tear-off de construtor | `ConstructorReference` → função | `propriedade` (`:1283-1312`) só acerta alvo identificador simples | tratar `Resolved::Constructor` antes de avaliar o alvo (11 FP) |
| acesso a propriedade | §2.5 | `acesso_a_propriedade` (`:1315-1336`): compara `alvo.tipo == core.string` | igual na regra; `{1}` usa `formatar` (com alias) — o oficial passa `String` sem alias |
| identificador | tabela de §2.4 | `valor_constante` (`:1340-1414`) | typedef → `Tipo(None)` (oficial: tipo instanciado nos limites); função estática ok; não resolvido depende de `tipos_invalidos` (T4) |
| deferred | `_getDeferredLibraryError` | ausente | implementar (§2.6) |
| criação: argumentos | `_valueOf` + objeto inválido | `valor_de` (`:1775-1786`) | igual; depende de `nao_resolvida` correto |
| `_checkFields` | tipo substituído para o teste, **declarado** para a marca de execução | `:1936-1943`: marca sobre o substituído | usar o declarado (1 FN + 1 FP) |
| `_checkParameters` | padrão de `super.x` herdado | `:1975-1982`: `null` | herdar (2 FP) |
| fronteiras de construtor | contextos + `copyWithEntity` | `completar_gerador`/`terminar_gerador` (`:2084`, `:2128`, `:2206`) recolocam no uso | faltam os textos de contexto |
| conversão de exceção | novo erro com contextos na criação | `formatar_erro_de_construtor` (`:590-599`) | sem contexto "The exception is…" |
| `fromEnvironment` | §3.2 | `chamar_factory` (`:1843-1890`) | bloqueado antes por `const_` vindo do patch (§D/T3) |
| valores (`valor.rs`) | §2.3, §4 | `somar` `:170`, `aritmetica` `:216`, `dividir_inteiro` `:302` (IDBZE com `de_execucao`), `resto` `:292`, deslocamentos `:408-441`, `comprimento` `:497` | conferidos nos códigos lançados; diferenças não encontradas nesta leitura (comparação linha a linha de `comparar`/`bits` **não verificada**) |
| relato | `_reportError` com a fonte corrente | `relatar_invalida` (`verificador.rs:151-160`): descarta erro de outra unidade | ver `const_initialized…` |

#### 9. Correções ao texto existente e pendências

Correções a §D (rodada 3):
1. `recursive_compile_time_constant`: o relato não é o `atElement` de `generateCycleError` (vai a um relator
   descartado); é o verificador lendo o `InvalidConstant` guardado — por isso `values` sintético não relata (§1.6).
2. `const_initialized_with_non_constant_value_from_deferred_library`: o erro fica no nome logo depois do
   prefixo (`E` em `self.E.f`), não no identificador final (§2.6).
3. `const_eval_type_string`: `valor.rs::comprimento` já lança `CONST_EVAL_TYPE_STRING` para o objeto inválido;
   a causa do FN é o identificador indefinido não marcado como tipo inválido.
4. "Estado do código": as edições listadas não estão no disco (revertidas em `fa8cfa96`); a tabela de §8 vale
   para o que está no disco.
5. T8/§D citam `evaluation.dart:223-325 (Tarjan)`: o Tarjan está em
   `_fe_analyzer_shared/lib/src/util/dependency_walker.dart:24-122`; `evaluation.dart:223-326` é só
   `computeDependencies`. A tarefa cita `DirectedGraph` de `utilities_collection.dart`: não é usado pelo motor
   de constantes no 3.6.2 (nenhuma referência a `DirectedGraph` em `analyzer/lib/src`).
6. Mensagens de contexto: no 3.6.2 o construtor sem nome aparece como `'A'`, não `'A.new'`.

Não verificado:
- ordem relativa dos alvos vindos do `HashSet` de `ConstantExpressionsDependenciesFinder` e das unidades em
  `_libraryFiles` (afeta só o falso positivo de §1.4-3);
- `field.type` em `_checkInitializers` (`getter.variable2`) ser ou não o tipo substituído;
- relato de erro nascido em outra unidade (offset de outro arquivo relatado na fonte corrente);
- amostras de oráculo 3.13.4 (atalhos de ponto, construtores primários, `const new`): não rodadas no oráculo vivo;
- causas no DartForge marcadas "não verificada"/"conforme §D" (`method/not_found_test.dart:11:24`,
  `case_expression_with_assignment*`, `multiline_newline_test`, `fromEnvironment` com patch,
  `ExperimentalMemberUse`, resolução de argumentos de anotação);
- valor exato de `0.0 == -0.0`/NaN além do que o fonte diz (o oráculo só mostra que não há erro).

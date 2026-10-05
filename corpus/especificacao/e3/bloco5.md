
#### E.3.7 Os códigos desta parte

##### `extraneous_modifier` (perda 6: FN 5, FP 1)
- **Emissão:** parser fasta. `ModifierContext.reportExtraneousModifier` (`mc:648`) e
  `reportTopLevelModifierError` (`mc:657`), chamados pelos `parse*Modifiers` e pelos `_parseCovariant`/
  `_parseFinal`/`_parseStatic`/`_parseVar` depois de `factory`; mais os relatos diretos de `pi:3624`,
  `3628`, `3636`, `4785`, `4828`, `4865`, `4874`, `5125`, `5129`, `8123`, `8130`, `8133`; e o AstBuilder
  em `var super.x` (`ab:1932`). Modelo `ExtraneousModifier` (`fm:1340`, índice 77).
- **Condição exata:** as tabelas de §E.3.4 (contexto × modificador, e "fora do `ModifierContext`"),
  com M1 (só quando o contexto é acionado) e M3 (`required` de nomeado relatado quando o contexto é
  acionado depois dele).
- **Posição:** o token do modificador (offset e length do lexema). `get`/`set` nos ramos mortos.
- **Mensagem:** `Can't have modifier '{0}' here.` / `Try removing '{0}'.`; `{0}` = lexema.
- **Supressões e ordem:** os relatos de `_parseModifiers` (ordem, repetição, conflito) saem antes, na
  ordem dos tokens; os "extraneous" depois, na ordem das chamadas do método. Um modificador não
  guardado (segundo `const`, `covariant` depois de `static`…) não gera "extraneous". `const class`,
  `external class/enum/typedef`, `final enum/mixin` têm código próprio. Modificador que é o primeiro
  token e manda para o membro de topo (`late mixin`, `var typedef`, `const enum`) não gera este código:
  gera o campo sem nome (§E.3.1).
- **No DartForge:** `crates/frontend/src/parser/modificadores.rs:118` (`modificador_estranho`), chamado
  de `:384-387` (topo), `:420` (membro), `:434-471` (parâmetro), `:481-509` (antes de palavra de
  topo), `declarations.rs:2250-2280` (método), `:2677-2678` (factory). Amostras:
  - FN `primary_constructors/syntax/const_in_header_and_body_error_test.dart:9:3`, `16:3`, `23:3`,
    `31:3` (`const this : assert(…);`, oráculo 3.13.4): **[main]** `parser_impl.dart:5575-5600` — na
    parte `this` do construtor primário, `var/final/const`, `external`, `static`, `covariant`, `late`
    são cada um `EXTRANEOUS_MODIFIER`. `declarations.rs:2431-2438` lê os modificadores e chama
    `parse_parte_primaria` sem relatar. Mudança: relatar os cinco, nessa ordem, antes de `:2438`.
  - FN `linguagem-3.13/nnbd/syntax/required_modifier_error_test.dart:32:3` (`required required int
    i`): M3, igual no 3.6.2 (`c18`). `types.rs:691-696` lê tudo pelo contexto e passa
    `opcional_nomeado = true` → nunca relata. Mudança: reproduzir o caminho rápido de
    `parseFormalParameter` (`[required se nomeado] [covariant se o dono aceita] [var|final]`); se ainda
    sobra modificador e o `required` foi consumido no caminho rápido, chamar
    `relatar_modificadores_de_parametro` com `opcional_nomeado = false`.
  - FP `primary_constructors/syntax/empty_body_error_test.dart:18:6` (`enum const E3;`, 3.13.4 dá
    `unexpected_token` `'const'`): `declarations.rs:1062` relata `EXTRANEOUS_MODIFIER` e devolve erro.
    **[main]** `parser_impl.dart:3877-3905`: `const` sem construtor primário é `UNEXPECTED_TOKEN` com o
    recurso desligado e `CONST_WITHOUT_PRIMARY_CONSTRUCTOR` com ele ligado, e a declaração segue.
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  void f(static int a, const int b, late int c, external int d, abstract int e) {}
  ```
  `extraneous_modifier` · 7+6 · 1:8 (`static`); 21+5 (`const`); 34+4 (`late`); 46+8 (`external`);
  62+8 (`abstract`) — todos `Can't have modifier '…' here.` (`c70`).
  ```dart
  void f({required required int a}) {}
  ```
  `extraneous_modifier` · 8+8 · 1:9 · Can't have modifier 'required' here.; `duplicated_modifier` ·
  17+8 · 1:18 · The modifier 'required' was already specified. (`c18`).

##### `static_constructor` (perda 1: FN 1)
- **Emissão:** parser, `parseMethod` (`pi:5012`); modelo `StaticConstructor` (`fm:1453`, índice 4).
- **Condição exata:** `isConstructor` (§E.3.5) e `staticToken != null`. `staticToken` é anulado antes
  se o membro é operador (`STATIC_OPERATOR`). Vale para construtor nomeado, de enum e de extension type
  (`c46`, `d69`, `e14`). Factory com `static` é `EXTRANEOUS_MODIFIER` (`c54`), não este código.
- **Posição:** o token `static` (length 6). **Mensagem:** `Constructors can't be static.`
- **Supressões e ordem:** sai depois dos erros do corpo; `static` torna `allowAbstract` falso, logo
  `static A();` dá também `missing_function_body` no `;` (`c01`); ordem entre os de construtor:
  nome errado, **static**, getter/setter, tipo de retorno, inicializador externo.
- **No DartForge:** `declarations.rs:2702-2704` (`parse_constructor`). FN
  `constructor/unnamed_new_error_test.dart:79:3` (`static void new() {}`, oráculo gravado 3.13.4:
  `static_constructor` 79:3, `constructor_with_return_type` 79:10, `experiment_not_enabled` 79:15):
  **[main]** `parser_impl.dart:5422` — `new` logo depois do **tipo** abre construtor. `declarations.rs:
  2439` só reconhece `new` logo depois dos modificadores. O 3.6.2 lê outra coisa (`d33`: campo `new`).
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  class A { static A(); }
  ```
  `static_constructor` · 10+6 · 1:11 · Constructors can't be static.; `missing_function_body` · 20+1 ·
  1:21 · A function body must be provided. (`c01`).

##### `constructor_with_return_type` (perda 1: FN 1)
- **Emissão:** parser, `parseMethod` (`pi:5022`); modelo `ConstructorWithReturnType` (`fm:2097`).
- **Condição exata:** `isConstructor && typeInfo != noType`. Entra aqui todo "método" com tipo cujo
  nome é o da classe (`void A()`), qualificado (`int A.foo()`, `void A.x()`) ou seguido de `:`
  (`void m() : x = 1 {}` → também `INVALID_CONSTRUCTOR_NAME`, `e38`).
- **Posição:** `beforeType.next`: **só o primeiro token** do tipo — `List` em `List<int> A()` (`e01`:
  10+4), o prefixo em `core.int A()` (`e02`: 38+4). **Mensagem:** `Constructors can't have a return type.`
- **Supressões e ordem:** `factory` com tipo antes é `TYPE_BEFORE_FACTORY` (no último token do tipo,
  `d107`). Depois de `STATIC_CONSTRUCTOR` (`c183`).
- **No DartForge:** `declarations.rs:2461-2474` (`constructor_com_retorno`, `:2588`): o intervalo é o
  **tipo inteiro** (`span_from(tstart)`), não o primeiro token — acerta só tipos de um token; com
  `List<int> A()` ou `p.T A()` erra a length (sem amostra no placar). O ramo não cobre `T nome(...) :
  inits` (construtor por causa do `:`). FN `unnamed_new_error_test.dart:79:10`: ver `static_constructor`.
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  class A { void A(); }
  ```
  `constructor_with_return_type` · 10+4 · 1:11 · Constructors can't have a return type. (`c02`).

##### `external_constructor_with_field_initializers` (perda 1: FN 1)
- **Emissão:** **AstBuilder**, `_buildConstructorDeclaration` (`ab:5911-5920`), chamado por
  `endClassConstructor`/`endMixinConstructor`/`endExtensionTypeConstructor`/…; não é modelo relatado
  pelo parser (o modelo `ExternalConstructorWithFieldInitializers`, `fm:1257`, só dá o código).
- **Condição exata:** `modifiers.externalKeyword != null`; para cada `p` em `parameters.parameters`,
  se `p.notDefault is FieldFormalParameterImpl` → um relato. `super.x` não conta. Factory não passa
  por aqui (`_buildFactoryConstructorDeclaration`).
- **Posição:** `notDefault.thisKeyword` (length 4), mesmo com tipo antes (`int this.x` → no `this`, `e06`).
- **Mensagem:** `An external constructor can't initialize fields.`
- **Supressões e ordem:** um por parâmetro (`e05`: dois). Independente de
  `EXTERNAL_CONSTRUCTOR_WITH_INITIALIZER` (parser, no `:`, para `external A() : x = 1;`).
- **No DartForge:** não existe (só o código em `crates/diagnostics/src/codigos_g.rs:2966`). FN
  `unsorted/external_test.dart:58:20` (`external Foo.n24(this.x);`, reproduzido em `d04`). Mudança: em
  `parse_constructor_resto` (`declarations.rs:2831`), com `mods.external && !factory`, relatar no
  `this` de cada parâmetro `this_` (o token `this` está em `types.rs:714`; é preciso guardar o seu span).
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  class A { external A(this.x, {this.y}); int x; int? y; }
  ```
  `external_constructor_with_field_initializers` · 21+4 · 1:22 e 30+4 · 1:31 · An external constructor
  can't initialize fields. (`e05`).

##### `external_factory_redirection` (perda 1: FN 1)
- **Emissão:** parser, `parseFactoryMethod` (`pi:5146-5149`); modelo `fm:3958`, índice 85.
- **Condição exata:** depois dos parâmetros e do modificador `async`, `next == '='` e
  `externalToken != null`. O redirecionamento é lido normalmente em seguida.
- **Posição:** o `=` (length 1). **Mensagem:** `A redirecting factory can't be external.`
- **Supressões e ordem:** exclui `EXTERNAL_FACTORY_WITH_BODY` (ramos alternativos). Vale em extension
  type (`e43`). O verificador ainda relata o alvo (`redirect_to_invalid_return_type` etc.).
- **No DartForge:** não existe. `declarations.rs:2834-2858` trata o `=` e só relata
  `REDIRECTION_IN_NON_FACTORY_CONSTRUCTOR` quando não é factory. FN `unsorted/external_test.dart:63:33`
  (`d04`). Mudança: em `:2835`, `if factory && mods.external` → erro no `eq.span`.
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  class A { external factory A.n() = A._; A._(); }
  ```
  `external_factory_redirection` · 33+1 · 1:34 · A redirecting factory can't be external. (`c182`).

##### `missing_function_body` (perda 3: FN 3)
- **Emissão:** parser, `parseFunctionBody` (`pi:5415`; modelo `ExpectedBody` em `;`, `=`, `return` —
  `pi:5432/5440/5459`; modelo `ExpectedFunctionBody` via `ensureBlock(BlockKind.functionBody)`,
  `pi:5477`) e `skipFunctionBody` (`pi:5379/5393`). Conversor: `error_converter.dart:316`, sem argumentos.
- **Condição exata:** a tabela de `parseFunctionBody` em §E.3.1 e o `allowAbstract` de cada chamador.
- **Posição:** `;`, `=` ou `return` (o próprio token); no caso geral, o token **seguinte** ao fim do
  cabeçalho, sem consumi-lo — EOF dá length 0 (`c08`: 9+0, linha 2 coluna 1).
- **Mensagem:** `A function body must be provided.` (o texto do CFE com `#lexeme` não é usado).
- **Supressões e ordem:** `external` no topo e `external`/não estático em membro dispensam o corpo
  `;`. `NON_SYNC_ABSTRACT_METHOD` do CFE é descartado pelo conversor (`:519`) "seguido de
  MISSING_FUNCTION_BODY". Depois de `MISSING_*_PARAMETERS` e antes dos erros de construtor.
- **No DartForge:** `declarations.rs:3091-3096` (`conferir_corpo_vazio`, no `;`) e `:3172-3193`
  (`parse_body_after_modifier`: `=`, `return`, palavra solta, caso geral). Amostras:
  - FN `augmentation_return_type_mismatch/AugmentationReturnTypeMismatch__topLeve_549a3325.dart:4:13`
    e `…_cd3d76ae.dart:6:13` (`augment core.int foo();`, reproduzidos em `d01`/`d02`): sem o
    experimento, `augment` é o tipo e `core` o nome; o token depois do nome é `.` → ramo de **método**
    (`pi:3617`) → `missing_function_parameters` em `core`, corpo: `.` → `missing_function_body` no `.`;
    a declaração acaba ali; o `.` solto é `expected_executable`; `int foo();` é outra função sem corpo.
    `declarations.rs:2063-2064` só toma o ramo de método com `(`, `<`, `{`, `=>`. Mudança: incluir `.`
    (topo: função sem parâmetros; membro: `parseQualifiedRestOpt`, construtor).
  - FN `missing_required_param/MissingRequiredParam__constructor_field_ce0ca2ef.dart:7:1`
    (`A({required this.})` + `}`; `d03`): o fasta põe nome sintético depois do `this.`
    (`missing_identifier` no `}`), fecha a lista e lê o corpo: `}` → `missing_function_body` nele.
    `types.rs:718` devolve erro (`expect_identifier()?`) e o membro inteiro é abandonado. Mudança: nome
    sintético ali, como já faz `types.rs:723-728` para parâmetro sem nome.
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  void f() = 1;
  ```
  `missing_function_body` · 9+1 · 1:10 · A function body must be provided. (`c132`; em `void f() return
  1;` 9+6, `c133`; em `class A { A() }` 14+1 no `}`, `c28`).

##### `missing_function_parameters` (perda 2: FN 2)
- **Emissão:** parser, `parseGetterOrFormalParameters` (`pi:1561`) e
  `parseFormalParametersRequiredOpt` (`pi:1758`), por `missingParameterMessage` (`pi:1842`).
- **Condição exata:** não é getter, e depois do nome (e dos parâmetros de tipo) não vem `(`, com
  `MemberKind` ∉ {`StaticMethod`, `NonStaticMethod`, `FunctionTypeAlias`}: função de topo, membro de
  **extension** e de **extension type** (`e34`), factory (`pi:5139`), função local e expressão de função
  (`pi:5242`, `5291`; não verificadas no oráculo). Em parâmetro-função (`pi:2123`) o `(` já foi visto
  antes: o relato não é alcançável (`void h(int k<T>) {}` dá só `expected_token ')'`, `e47`).
- **Posição:** topo/extension: o **nome** (operador: o símbolo). Factory e função local: o token
  **seguinte** (`class A { factory A }` → no `}`, `c129`). Nome sintético → o token seguinte.
- **Mensagem:** `Functions must have an explicit list of parameters.`
- **Supressões e ordem:** parênteses sintéticos são inseridos e o corpo é lido: `f {}` não tem mais
  nada (`c09`); sem corpo plausível segue `missing_function_body`.
- **No DartForge:** `declarations.rs:2068-2075` (função/método), `:2176-2183` (setter). FN
  `AugmentationReturnTypeMismatch__topLeve_549a3325.dart:4:9` e `…_cd3d76ae.dart:6:9`: mesma causa do
  código anterior (nome seguido de `.`). Factory sem `(` não é tratada (`parse_formal_parameters()?`
  em `:2831` falha com outro erro).
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  foo.bar() {}
  ```
  `missing_function_parameters` · 0+3 · 1:1 · Functions must have an explicit list of parameters.;
  `missing_function_body` · 3+1 · 1:4; `expected_executable` · 3+1 · 1:4 (`d43`; `bar() {}` fica válido).

##### `missing_method_parameters` (perda 2: FP 2)
- **Emissão:** parser, `parseGetterOrFormalParameters` (`pi:1561`) com `MemberKind.StaticMethod` ou
  `NonStaticMethod` — membros de **classe, mixin e enum** (`pi:4943-4949`).
- **Condição exata:** em `parseMethod`, não é (considerado) getter e não vem `(`. Um getter deixa de
  ser considerado getter se tem nome qualificado ou `:` depois (`pi:4931-4934`).
- **Posição:** o nome — o **primeiro** identificador (`A` em `A.foo;`, `d58`); operador: o símbolo
  (`operator +;` → no `+`, `c139`); nome sintético → o token seguinte (`=> 0;` → no `=>`, `d56`).
- **Mensagem:** `Methods must have an explicit list of parameters.`
- **No DartForge:** `declarations.rs:2069-2074`, `:2177-2182`, `:2417`, `:2477-2504`. FP
  `constructor/unnamed_new_error_test.dart:59:15` (`int get new => 1;`) e `:91:22` (`static int get new
  => 42;`): `get` seguido de palavra **reservada** e de `;`/`=`/`(`/`{`/`=>`/`<` é getter com nome
  recuperado (`pi:4609-4615`; só `expected_identifier_but_got_keyword` em `new`, `d33`/`d34`, igual no
  oráculo gravado 3.13.4). `declarations.rs:1863` (`accessor_follows`) exige identificador depois de
  `get`: o membro falha, a recuperação para em `new`, e o `=>` cai em `:2414-2418` (identificador e
  parâmetros que faltam no `=>`). Mudança: aceitar a palavra reservada como nome em `accessor_follows`
  quando o token seguinte "indica método ou campo".
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  class A { void m {} }
  ```
  `missing_method_parameters` · 15+1 · 1:16 · Methods must have an explicit list of parameters. (`c34`).

##### `expected_executable` (perda 2: FN 2)
- **Emissão:** parser, `parseInvalidTopLevelDeclaration` (`pi:9468`) e o ramo sem progresso de
  `parseUnit` (`pi:431`); modelo `ExpectedDeclaration` (`fm:875`); conversor `:115`, sem argumento.
- **Condição exata:** depois da metadata, o token não é palavra de topo, modificador, palavra/
  identificador, `(`, nem operador seguido de `(`; e não é `;` (que é `UNEXPECTED_TOKEN`). Também
  quando `parseTopLevelMemberImpl` não leu nada e o "nome" não é palavra (`pi:3593`).
- **Posição:** o token (EOF: length 0). Consome um token; `{…}` inteiro, calado.
- **Mensagem:** `Expected a method, getter, setter or operator declaration.`
- **No DartForge:** `declarations.rs:175-176` (`can_start_declaration`), `:2023-2024`, e
  `recover_top_level` (`:326`). FN `primary_constructors/syntax/empty_body_error_test.dart:35:20` e
  `:44:34` (`class C = S with M {}`): `parse_class:998` chama `garantir_ponto_e_virgula`, que diante de
  `{` relata o `;` mas devolve erro (`mod.rs:546-548`), e `recover_top_level` engole o `{}` sem relato.
  Mudança: `garantir_ponto_e_virgula_forcado` ali; e no topo, `{` solto consome o bloco casado depois
  de relatar (hoje `{ var x = 1; }` daria um segundo relato no `}` — o fasta dá um só, `c42`).
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  class S {}
  mixin M on S {}
  class C = S with M {}
  ```
  `expected_token` · 44+1 · 3:18 · Expected to find ';'.; `expected_executable` · 46+1 · 3:20 ·
  Expected a method, getter, setter or operator declaration. (`d35`).

##### `expected_class_member` (perda 6: FP 6)
- **Emissão:** parser, `recoverFromInvalidMember` (`pi:9386`); modelo `fm:879`; conversor `:108`.
- **Condição exata:** linha 6 da tabela de §E.3.3: o "nome" não é identificador (nem `class`, `enum`,
  `typedef`, operador, `(`, `=>`, `{`) **e nada foi lido antes dele** (`token == beforeStart`: sem
  modificador nem tipo; metadata não conta).
- **Posição:** o token; consumido, salvo `}`. **Mensagem:** `Expected a class member.`
- **No DartForge:** `declarations.rs:2426-2428` e `recover_member` (`:427`). FP
  `constructor/unnamed_new_error_test.dart:71:11`, `71:13`, `71:14` (`int new = 1;`) e `:103:18`,
  `103:20`, `103:21` (`static int new = 1;`): `declarations.rs:1952-1957` lê "identificador seguido de
  palavra-chave" como campo sem tipo (`int`), o `new` falha sozinho e `=`, `1`, `;` viram três
  `expected_class_member`. Regra do fasta (`pi:4743-4753`; topo `pi:3540-3553`): sem tipo e sem
  `var/final/const`, identificador + palavra **reservada** + (`;` `=` `(` `{` `=>` `<`) → o
  identificador é o tipo e a reservada o nome (`expected_identifier_but_got_keyword` nela, só isso:
  `d33`/`d34`); só quando esse terceiro token não "indica método ou campo" o identificador é o nome
  (`augment class X {}`). Corrige também os FP de `missing_const_final_var_or_type` desse arquivo.
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  class A { 42 }
  ```
  `expected_class_member` · 10+2 · 1:11 · Expected a class member. (`c38`).

##### `expected_body` (perda 3: FN 3)
- **Emissão:** parser, `ensureBlock` (`pi:4200`) com `BlockKind` de `message`: classe (`pi:2722`),
  mixin (`2954`), extension (`3178`), extension type (`3249`), switch (`8977`), switch-expressão
  (`10362`), try/catch/finally (`8823/8929/8939`). Nove códigos únicos com o mesmo nome (§E.3.0).
- **Condição exata:** depois do cabeçalho (e da sua recuperação) o token seguinte não é `{`.
- **Posição:** o **último token consumido** (não o seguinte): o nome ou o último token da última
  cláusula; no `switch`, o `)`. Sintético de largura 0 → o seguinte.
- **Mensagem:** por espécie: `A class declaration must have a body, even if it is empty.`, `A mixin
  declaration…`, `An extension declaration…`, `An extension type declaration…`, `A switch statement…`…
- **Supressões e ordem:** enum não usa este código (`missing_enum_body`, no token seguinte); função
  também não (`missing_function_body`). O corpo sintético é vazio e o que vem depois é lido como
  declarações/comandos seguintes — em `switch(i) L: { case 111: … }` o `L: {…}` vira bloco rotulado e
  `case` dá a cascata de `d05`.
- **No DartForge:** `declarations.rs:1105-1115` (classe, mixin, extension, extension type). Amostras:
  - FN `label/label8_test.dart:8:11` (`d05`): o código de switch não existe em
    `crates/frontend/src/parser/statements.rs` (nenhuma referência a `EXPECTED_SWITCH_STATEMENT_BODY`).
  - FN `test_runner/impl/imported_library_part.dart:7:7` e `test_runner/impl/part.dart:3:7`: o parser
    acerta (`imported_library.dart:5:7` bate); os dois arquivos são `part of` de bibliotecas que não
    estão no corpus e não entram na análise (`crates/paridade/src/analise.rs:192-199` só importa o que
    não é parte). O `dart analyze` analisa a parte órfã (`d06`). Mudança fora do parser: analisar
    partes órfãs como unidade solta.
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  part of 'nao_existe.dart';

  class A
  ```
  `expected_body` · 34+1 · 3:7 · A class declaration must have a body, even if it is empty. (`d06`).

##### `representation_field_modifier` (perda 12: msg 11, FP 1)
- **Emissão:** 3.6.2: AstBuilder `endPrimaryConstructor` (`ab:2886-2893`). 3.13.4: `ErrorVerifier`
  (**[main]** `error_verifier.dart:5684-5693`, linha 9 da tabela de §E.3.6).
- **Condição exata:** 3.6.2: o 1º parâmetro é simples e tem `keyword` ≠ `const` (`final` ou `var`),
  qualquer que seja o número de parâmetros. 3.13.4: exatamente um parâmetro, posicional, não função,
  com `final` ou `var` (só `var` com `primary-constructors` ligado).
- **Posição:** a palavra `final`/`var`.
- **Mensagem:** 3.6.2 `Representation fields can't have modifiers.` (`am:21564`); 3.13.4
  `Representation fields can't have the modifier 'var'.` — texto fixo, sem argumento.
- **No DartForge:** `declarations.rs:1736-1745`. msg ×11 (`extension_type_error_test.dart:14:20`,
  `20:20`, `68:29`, `73:30`, `109:21`; `covariant_with_final_error_test.dart:27:29`;
  `final_formal_parameter_error_test.dart:33:19`; `var_formal_parameter_error_test.dart:51:19`;
  `regress_53625_error_test.dart:32:20`, `38:20`, `46:20`): todos de oráculo 3.13.4; falta escolher o
  texto por T2 do §0. FP `extension_type_error_test.dart:29:20` (`ET3(final i, final x)`): a guarda
  `varios_no_3_13` (`:1736`) usa `versao() > PISO`, mas o arquivo é versão 3.6 julgado pelo 3.13.4
  (sintaxe nova): tem de usar o mesmo predicado de oráculo de T1 do §E.0, não a versão da biblioteca.
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  extension type E(final int i) {}
  ```
  `representation_field_modifier` · 17+5 · 1:18 · Representation fields can't have modifiers. (`c20`).

##### `expected_representation_type` (perda 5: FN 4, pos 1)
- **Emissão/condição:** 3.6.2 `ab:2871-2877`: 1º parâmetro simples com `type == null`. 3.13.4:
  linha 10 de §E.3.6 (um só parâmetro, posicional, não função, sem tipo).
- **Posição:** 3.6.2 `leftParenthesis.next` (1º token depois do `(`: `@`, `final`, `covariant`, o
  nome). 3.13.4: **o nome** do parâmetro. **Mensagem:** `Expected a representation type.`
- **No DartForge:** `declarations.rs:1725-1733`: posição `depois_abre` (regra 3.6.2) e, com
  `final`/`var`, **não relata** (`:1728`, comentário "o 3.13.4 aceita sem tipo" — falso sem o recurso
  ligado). Amostras, todas de oráculo 3.13.4: FN `extension_type_error_test.dart:20:24` (`ET2(var i)`),
  `regress_53625_error_test.dart:46:26` (`E04(final x)`) e o `:38:24` (`E03(var x)`) que o placar
  mostra como "posição" → relatar no nome mesmo com `final`/`var`; o "nosso 186:20" que o placar casa
  com o 38:24 é o relato de `E26(@anno int)` no `@` (`depois_abre`), cujo par certo é o FN `:186:26`
  (o 3.13.4 relata no nome `int`); FN `:192:20` (`E27(int @anno x)`): `int` é o nome sem tipo e
  depois falta `)` — `types.rs` falha no `@` e a representação não é examinada.
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  extension type E(@deprecated i) {}
  ```
  `expected_representation_type` · 17+1 · 1:18 · Expected a representation type. (`c103`, no `@`).

##### `expected_representation_field` (perda 4: FN 2, pos 2)
- **Emissão/condição:** 3.6.2 `ab:2911-2914`: o 1º parâmetro não é `SimpleFormalParameter` (lista
  vazia, `this.x`, `super.x`, função, grupo opcional/nomeado, posicional com `= valor`). 3.13.4: linhas
  1, 3, 4, 5, 7, 8 de §E.3.6.
- **Posição:** 3.6.2 `leftParenthesis.next` (length 0 se a lista é sintética). 3.13.4: `)`, `this`,
  `super`, o delimitador, ou o 1º token do parâmetro-função. **Mensagem:** `Expected a representation field.`
- **No DartForge:** `declarations.rs:1719-1722` (em `depois_abre`). pos
  `regress_53625_error_test.dart:65:24` (`E07(int this.x)`) e `:78:24` (`E09(int super.x)`): o 3.13.4
  relata no `this`/`super`, nós em `int`. FN
  `AugmentationModifierExtra__extensionTyp_72995408.dart:3:26` e
  `AugmentationModifierMissing__extensionT_f07aa47f.dart:3:26` (`augment extension type E {`): sem
  construtor primário o 3.13.4 relata length 0 no offset do token seguinte ao nome; `declarations.rs:
  1636-1650` só emite `MISSING_PRIMARY_CONSTRUCTOR`; para oráculo 3.13.4 falta o segundo relato.
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  extension type E.n {}
  ```
  `expected_representation_field` · 16+0 · 1:17 · Expected a representation field.;
  `missing_primary_constructor_parameters` · 17+1 · 1:18 (`c91`). `extension type E() {}` → 17+1 (`c19`).

##### `const_primary_constructor_with_body` (perda 3: FN 3)
- **Emissão:** só 3.13.4. **[main]** `ErrorVerifier._validateConstructorBodyAllowed`
  (`analyzer/lib/src/generated/error_verifier.dart:8941-8998`), chamado de
  `visitPrimaryConstructorDeclaration` (`:2112-2120`) com `isPrimary: true`. Dois códigos únicos
  (`constPrimaryConstructorWithBlockBody`/`…ExpressionBody`, [main] `am:23388-23399`,
  `type: compileTimeError`). Não existe no 3.6.2.
- **Condição exata [main]:** há parte `this` com corpo; o construtor **não** é factory; `element.isConst`
  (o `const` do cabeçalho; enum sempre); corpo é bloco ou `=>`. Só roda se
  `_checkForConstConstructorWithNonConstSuper` não relatou (`:2102`).
- **Posição:** o `{` (bloco) ou o `=>`. **Mensagem:** `The body part of a constant primary constructor
  can't have a block body.` / `…an expression body.`
- **No DartForge:** `declarations.rs:1301-1311`, dentro de `elaborar_construtor_primario`, que só é
  chamado para classe (`:1032`) e enum (`:1504`). FN
  `constructor_body/ConstructorBody__extensionType_primaryC_2a25fda3.dart:2:8`,
  `…_a68b6b78.dart:2:8`, `primary_constructors/syntax/const_with_body_error_test.dart:37:8` — todos
  `extension type const E(…) { this {} }`: `parse_extension_type` (`:1614-1667`) não elabora a parte
  `this`. Mudança: aplicar a mesma verificação (com o `const_` de `:1617`) aos membros do extension type.
- **Exemplos:** não há no 3.6.2; ali `this {}` em corpo é `expected_class_member` 35+4 em `this`,
  `missing_identifier` e `missing_function_parameters` 40+1 no `{` (`d31`). Oráculo gravado 3.13.4
  (`ConstructorBody__extensionType_primaryC_2a25fda3.dart`): `experiment_not_enabled` 35+4 2:3;
  `const_primary_constructor_with_body` 40+1 2:8.

##### `extraneous_modifier_in_primary_constructor` (perda 3: FN 3)
- **Emissão:** parser, `ModifierContext.parseFormalParameterModifiers` (`mc:222-223`,
  `reportExtraneousModifierInPrimaryConstructor`, `mc:693`); modelo `fm:1418`, índice 175. **Existe no
  3.6.2**, onde `MemberKind.PrimaryConstructor` só ocorre em extension type.
- **Condição exata:** parâmetro de construtor primário com `covariant` (o caminho rápido não o
  consome, `pi:1962`). **[main]** `modifier_context.dart:234-249`: com `primary-constructors`
  **desligado**, igual; ligado, vira `INVALID_COVARIANT_MODIFIER_IN_PRIMARY_CONSTRUCTOR` e só quando
  não há `var`.
- **Posição:** o `covariant` (length 9). **Mensagem:** `Can't have modifier 'covariant' in a primary
  constructor.` (`{0}` = lexema).
- **Supressões e ordem:** no 3.6.2 o `covariant` continua sendo `leftParenthesis.next` para os
  relatos de representação (`e28`).
- **No DartForge:** `modificadores.rs:450-455` (extension type) e `declarations.rs:1235-1241` (classe
  e enum, só `covariant` **sem** `var`). FN `invalid_override/InvalidOverride__class_declaringFormalP_14faa66e.dart:4:9`,
  `…_ba98c91d.dart:8:9`, `invalid_use_of_covariant/InvalidUseOfCovariant__class_primaryConstructor.dart:1:9`
  — todos `class C(covariant var T x)` em oráculo 3.13.4 com o recurso desligado: sai mesmo com `var`.
  Mudança: `:1235` relata sempre que `!features.tem(PrimaryConstructors)`.
- **Exemplos (oráculo vivo 3.6.2):**
  ```dart
  extension type E(covariant int i) {}
  ```
  `extraneous_modifier_in_primary_constructor` · 17+9 · 1:18 · Can't have modifier 'covariant' in a
  primary constructor. (`c74`).

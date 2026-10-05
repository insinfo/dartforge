### E.3 Declarações, membros e modificadores (rodada 4)

Fonte: parser fasta e AstBuilder do **3.6.2**, lidos nesta sessão. Abreviações desta parte:
`pi:` = `_fe_analyzer_shared/lib/src/parser/parser_impl.dart`, `mc:` =
`_fe_analyzer_shared/lib/src/parser/modifier_context.dart`, `fe:` = `_fe_analyzer_shared/lib/src/`,
`ab:` = `analyzer/lib/src/fasta/ast_builder.dart`, `an:` = `analyzer/lib/src/`, `fm:` =
`pkg/front_end/messages.yaml`, `am:` = `pkg/analyzer/messages.yaml`. O que vem do checkout
main/3.14 (`E:\references\dart-sdk`) está marcado **[main]** e serve só para os arquivos julgados
pelo 3.13.4; quando há, cito o **oráculo gravado** (`corpus/diagnosticos/<grupo>/oraculo.jsonl`,
saída do 3.13.4) porque não há binário 3.13.4 nesta máquina. Todo exemplo "entrada → diagnósticos"
foi rodado no oráculo vivo 3.6.2 (`C:\tools\dartsdk-3.6.2`, casos em
`E:\dftemp\analise\spec-r4\casos\e3\{a,b}\`); formato das saídas: `código · offset+length ·
linha:coluna · mensagem`. Perdas de `familia-E.txt`, amostras de `placar-r7.txt`.

Correções ao texto existente da família E (§E, r3-e), detalhadas nos códigos:
- `extraneous_modifier`: "`required required int i` no 3.13.4 (o 3.13 relata extraneous no 1º além de
  DUPLICATED) … fora da fonte 3.6.2" está **errado**: o 3.6.2 faz o mesmo (`c18`), é efeito de
  `parameterKind` trocado antes do `ModifierContext` (§E.3.4, regra M3).
- `expected_representation_type`/`_field`: "a posição é `leftParenthesis.next` … nós usamos o início do
  parâmetro" está **invertido** para as amostras: elas são de arquivos 3.13, onde o relato saiu do
  AstBuilder e foi para o `ErrorVerifier` com outra posição (nome do parâmetro, `this`, `super`, o
  delimitador); `leftParenthesis.next` é a regra do 3.6.2, e é a que o DartForge aplica hoje (§E.3.6).
- `representation_field_modifier`: a mensagem do 3.13.4 **não** tem argumento ("arg = lexema" está
  errado): o texto é fixo, `…can't have the modifier 'var'.`, até quando o token é `final`.
- `expected_class_member`: `unnamed_new_error_test` é julgado pelo **3.13.4** (está em
  `sintaxe-nova.json`), não pelo 3.6.2; e `int new = 1;` não é "construtor nomeado `new`": é um campo
  com nome reservado (`expected_identifier_but_got_keyword`), nas duas versões.
- `expected_body`: os dois FN de `test_runner/impl/*part*.dart` não são do parser: o arquivo é uma
  parte cuja biblioteca não está no corpus e nunca é carregada (`crates/paridade/src/analise.rs:192`).
- `const_primary_constructor_with_body` e `extraneous_modifier_in_primary_constructor`: o segundo
  **existe** no 3.6.2 (`fm:1418`, índice 175, `mc:693`); só o primeiro é posterior (§E.3.6).
- Cauda: `external_constructor_with_field_initializers` não é do parser (é do AstBuilder, nos
  parâmetros `this.x`), e é diferente de `external_constructor_with_initializer` (parser, no `:`).

#### E.3.0 Do relato do parser ao diagnóstico

- **Token do relato.** `reportRecoverableError(token, msg)` (`pi:9429`) e
  `reportRecoverableErrorWithToken` (`pi:9445`) trocam o token por `findNonZeroLengthToken`
  (`fe:parser/util.dart:62`): enquanto o token é sintético de largura 0 e não é EOF, anda para o
  **seguinte**. `reportRecoverableErrorWithEnd(início, fim, msg)` (`pi:9435`) não ajusta.
- **Offset e length.** `AstBuilder.handleRecoverableError` (`ab:5340`): `offset = startToken.offset`,
  `length = endToken.end - offset` — o lexema inteiro do token (EOF: length 0, ex. `c08`, `c24`).
  Relatos feitos pelo próprio AstBuilder usam `errorReporter.atToken(token, código)`: mesma conta, mas
  **sem** `findNonZeroLengthToken` — token sintético dá length 0 (`c91`).
- **Código.** `FastaErrorReporter.reportMessage` (`an:fasta/error_converter.dart:557`): se o modelo
  tem `index` em `fm:`, o código é `fastaAnalyzerErrorCodes[index]` e os argumentos são
  `message.arguments.values` na ordem de declaração; senão `reportByCode(analyzerCodes.first, …)`
  (`:27`), um `switch` por nome que monta os argumentos à mão (`UNEXPECTED_TOKEN` → `[lexema]`,
  `EXPECTED_TOKEN` → `[string]`; `EXPECTED_EXECUTABLE`, `EXPECTED_CLASS_MEMBER`,
  `MISSING_FUNCTION_BODY`, `MISSING_*_PARAMETERS` sem argumento: o `#lexeme` do modelo do CFE se perde).
- **Nome × nome único.** `expected_body` é o nome compartilhado de nove códigos
  (`an:dart/error/syntactic_errors.g.dart:550`…): o texto muda, o nome que o `dart analyze` imprime não.

| modelo fasta (`fm:` linha) | quem relata | código do analyzer | onde vai |
|---|---|---|---|
| `ExpectedDeclaration` (875) | `pi:432`, `pi:9474` | `expected_executable` | o token inesperado |
| `UnexpectedToken` (1774) | `pi:9473` (`;` no topo), cabeçalhos, corpo | `unexpected_token` `[lexema]` | o token |
| `ExpectedClassMember` (879) | `pi:9386` | `expected_class_member` | o token |
| `ExpectedBody` (592) | `pi:5379/5393/5432/5440/5459` | `missing_function_body` | `;`, `=`, `return` |
| `ExpectedFunctionBody` (883) | `ensureBlock(functionBody)` `pi:5477` | `missing_function_body` | o token **seguinte** |
| `ExpectedClassBody` (786, índice 8) | `pi:2722` | `expected_body` (class) | o **último token do cabeçalho** |
| `ExpectedMixinBody` (795) / `ExpectedExtensionBody` (804) / `ExpectedExtensionTypeBody` (813) | `pi:2954/3178/3249` | `expected_body` | idem |
| `ExpectedEnumBody` (3085) | `pi:2429` | `missing_enum_body` | o token **seguinte** |
| `ExpectedSwitchStatementBody` (864) | `pi:8977` | `expected_body` (switch) | o `)` |
| `ExpectedTryStatementBody`/`Catch`/`Finally` (822/833/844) | `parseBlock` | `expected_body` | o token anterior |
| `MissingFunctionParameters` (3300) | `pi:1561`, `pi:1758` | `missing_function_parameters` | nome (método/função) ou token seguinte (factory, parâmetro-função) |
| `MissingMethodParameters` (3307) | `pi:1561` | `missing_method_parameters` | nome |

A diferença "token anterior × token seguinte" vem de `ensureBlock` (`pi:4200`): `BlockKind` com
`message` (classe, mixin, extension, extension type, switch, try…) relata em `token` (o último
consumido); com `template` (função, enum) relata em `token.next`; sem `BlockKind` é
`EXPECTED_TOKEN '{'` em `token.next`. Em todos insere `{` `}` sintéticos de largura 0 no offset do
token seguinte (`insertBlock`, `pi:4220`) e **não consome nada**.

#### E.3.1 Laço de topo e recuperação

```
parseUnit(token)                                               // pi:403
  pula ErrorTokens (relatados no fim: reportAllErrorTokens, pi:440)
  directiveState = DirectiveContext(enhanced-parts?)
  se SCRIPT_TAG: checkScriptTag; parseScript
  enquanto !token.next.isEof:
    start = token.next
    token = parseTopLevelDeclarationImpl(token, directiveState); endTopLevelDeclaration
    se start == token.next:                                    // sem progresso (pi:424)
      token = token.next; EXPECTED_EXECUTABLE nele; handleInvalidTopLevelDeclaration

parseTopLevelDeclarationImpl(token, ds)                        // pi:538
  token = parseMetadataStar(token); next = token.next
  se next.isTopLevelKeyword → parseTopLevelKeywordDeclaration(begin=next, modifierStart=token, next…)
  se next.isModifier:                                          // Token.isModifier: NÃO olha o seguinte
    se next ∈ {var, late} | (final e seguinte ∉ {class,mixin,enum}) | (const e seguinte ≠ class):
       ds.checkDeclaration(); → parseTopLevelMemberImpl(token)  // pi:557-570
    enquanto token.next.isModifier: token = token.next          // só anda; nada relatado aqui
  next = token.next
  macro (seguido de class) | sealed | base | interface (seguidos de class/mixin/enum):
       guarda o token e next avança (pi:580-613); "sealed abstract class": modifierStart = sealed
  se next.isTopLevelKeyword → parseTopLevelKeywordDeclaration(beginToken, modifierStart, next, …)
  senão se next.isKeywordOrIdentifier → checkDeclaration; parseTopLevelMemberImpl(modifierStart)
  senão se modifierStart.next != next → idem (modificador usado como nome)       // pi:629
  senão se next == '(' → idem (tipo record)                                     // pi:633
  // recuperação
  se next.isOperator e next.next == '(':  TOP_LEVEL_OPERATOR em next; insere identificador
       sintético depois de next; → parseTopLevelMemberImpl(next)                 // pi:639 (c43)
  beginTopLevelMember(next); → parseInvalidTopLevelDeclaration(token)

parseInvalidTopLevelDeclaration(token)                         // pi:9468
  next = token.next
  relata em next: ';' → UNEXPECTED_TOKEN [';'] ; qualquer outro → EXPECTED_EXECUTABLE
  se next == '{': next = parseInvalidBlock(token)   // lê o bloco com os erros DESLIGADOS (pi:8647)
  handleInvalidTopLevelDeclaration(next); devolve next          // consome 1 token (ou o bloco inteiro)
```

Consequências conferidas:
- Token solto consome **um** token por erro: `42;` → `expected_executable` em `42` e
  `unexpected_token` em `;` (`c40`); `=> 0;` → dois `expected_executable` (`=>`, `0`) e
  `unexpected_token` (`d54`). Exceção: `{ … }` solto é **um** relato no `{` e o bloco inteiro é
  consumido sem relatar o que há dentro (`c42`).
- `}`, `)`, `]` soltos: `expected_executable` neles, um token (`c11`, `d40`, `d41`, `d42`, `d101`). O
  scanner não casa o fecho solto com nada; o `}` de classe que sobra (`class A {}}`) chega ao topo.
- Só metadata antes do EOF: `expected_executable` no EOF, length 0 (`c164`).
- `Token.isModifier` aqui é a propriedade do **tipo** do token (`fe:scanner/token.dart:626`), não a
  função `isModifier(token)` de `mc:12` (que exige palavra depois). `abstract`, `augment`, `const`,
  `covariant`, `external`, `final`, `late`, `required`, `static`, `var` têm a propriedade
  (`token.dart:107…328`); `augment` só chega como palavra-chave se `Feature.macros` está ligado
  (`fe:scanner/abstract_scanner.dart:1754`, `an:dart/scanner/scanner.dart:206`) — sem isso é
  identificador comum, daí `augment class X {}` ser o campo `augment` (`c23`, T1 do §0).
- `late mixin M {}`, `var typedef F = int;`, `const enum G {…}`, `final typedef H = int;` nunca
  chegam ao `ModifierContext`: a primeira palavra já mandou para `parseTopLevelMemberImpl`, que lê
  um campo sem nome — `missing_identifier` na palavra de topo, `expected_token ';'` no modificador
  (`c83`, `c84`, `d111`). Só chegam a `parse*Modifiers` os modificadores que **não** são o primeiro
  token do grupo `var/late/final/const` acima (`static class`, `abstract enum`, `external typedef`…).

`parseTopLevelKeywordDeclaration` (`pi:658`), por palavra:

| palavra | condição extra | faz |
|---|---|---|
| `class` | — | `checkDeclaration`; `parseClassModifiers`; `parseClassOrNamedMixinApplication` (`pi:802`) |
| `enum` | — | `checkDeclaration`; `parseEnumModifiers`; `BASE_ENUM`/`FINAL_ENUM`/`INTERFACE_ENUM`/`SEALED_ENUM` no token (`pi:685-696`); `parseEnum` |
| qualquer outra | seguinte é `(` ou `.` (e não é `typedef` + record + nome) | é **nome**: `parseTopLevelMemberImpl(modifierStart)` (`pi:719`) |
| qualquer outra | seguinte é `<` | idem, salvo `extension<T> on …` (`pi:723-736`) |
| `import`/`export`/`part` | — | `parseTopLevelKeywordModifiers`; `checkImport`/`checkExport`/…; diretiva |
| `typedef` | — | `parseTypedefModifiers`; `checkDeclaration`; `parseTypedef` |
| `mixin` | seguinte `class` | `_handleModifiersForClassDeclaration` com `mixinToken`: `FINAL_MIXIN_CLASS`, `INTERFACE_MIXIN_CLASS`, `SEALED_MIXIN_CLASS` (`pi:814-828`) |
| `mixin` | — | `parseMixinModifiers`; `FINAL_MIXIN`/`INTERFACE_MIXIN`/`SEALED_MIXIN` (`pi:766-775`); `parseMixin` |
| `extension` | — | `parseExtensionModifiers`; `parseExtension` |
| `library` | — | `checkLibrary`; `parseLibraryDirectiveModifiers`; com `augment` → `parseLibraryAugmentation` |

Ordem das diretivas (`fe:parser/directive_context.dart:9`; estados `Unknown < Script < Library <
ImportAndExport < Part < PartOf < Declarations`), relato sempre na palavra da diretiva:

| chega | estado | código |
|---|---|---|
| `import`/`export` | `Part` | `import_directive_after_part_directive` / `export_directive_after_part_directive` (`d110`) |
| `import`/`export`/`part` | `PartOf` sem enhanced-parts | `non_part_of_directive_in_part` |
| `import`/`export`/`part` | `Declarations` | `directive_after_declaration` (`c44`) |
| `library` | `Library` | `multiple_library_directives` (`d108`) |
| `library` | `PartOf` | `non_part_of_directive_in_part` |
| `library` | outro ≥ `ImportAndExport` | `library_directive_not_first` (`d109`) |
| `part of` | `PartOf` | `multiple_part_of_directives` |
| `part of` | outro ≠ `Unknown` | `non_part_of_directive_in_part` |

`AstBuilder.addProblem` (`ab:203`) troca `NON_PART_OF_DIRECTIVE_IN_PART` por
`DIRECTIVE_AFTER_DECLARATION` quando ainda não há diretiva na unidade. `checkDeclaration` não sai de
`PartOf`.

**Membro de topo** — `parseTopLevelMemberImpl(token)` (`pi:3414`):

```
(1) non-NNBD: `late` usado como modificador → UNEXPECTED_TOKEN (modelo UnexpectedModifierInNonNnbd)
(2) caminho rápido de modificadores (usa isModifier de mc:12):
      [external | augment]  [final | var | const | late [final]]
    se ainda há modificador:
      se já há var/final/const e o próximo é final/var/const: NÃO lê (é a declaração seguinte)
      senão ModifierContext.parseTopLevelMemberModifiers (§E.3.4)
(3) com var/final/const: skipOuterPattern + '=' → PATTERN_VARIABLE_DECLARATION_OUTSIDE_FUNCTION_OR_METHOD
      do 1º ao último token do padrão; nome sintético; parseFields (pi:3495-3519)
(4) typeInfo = computeType(token, required=false, inDeclaration=true); pula o tipo
(5) next ∈ {get,set} e next.next.isIdentifier → getOrSet
(6) sem tipo, sem var/final/const, next.next palavra RESERVADA e depois ; = ( { => < :
      relê o tipo como obrigatório (o identificador vira o tipo, a reservada o nome)  (pi:3540)
(7) nome não é IDENTIFIER puro:
      factory | operator (sem getOrSet) seguidos de algo ∉ {( { < => = ; ,}:
          FACTORY_TOP_LEVEL_DECLARATION / TOP_LEVEL_OPERATOR em next;
          (operator + operador + '(': anda até o operador e insere identificador sintético)
          handleInvalidTopLevelDeclaration; devolve next   (c157: 1 token; c158: 2; o resto segue)
      não é identificador nenhum:
          palavra-chave → segue (ensureIdentifier relata)
          token == beforeStart (nada lido) → parseInvalidTopLevelDeclaration
          senão insere identificador sintético (MISSING_IDENTIFIER) e segue
(8) depois = token após o nome (pulando um '!' de recuperação)
    se getOrSet != null ou depois ∈ { ( , { , < , . , => }:          // MÉTODO   (pi:3613)
       var → VAR_RETURN_TYPE ; final/const → EXTRANEOUS_MODIFIER ; senão late → EXTRANEOUS_MODIFIER
       parseTopLevelMethod
    senão                                                            // CAMPO
       parseFields(DeclarationKind.TopLevel)
```

`parseTopLevelMethod` (`pi:3856`): tipo, nome, parâmetros de tipo (`parseMethodTypeVar`),
`parseGetterOrFormalParameters(MemberKind.TopLevelMethod)`, modificador `async`,
`setter` não síncrono → `INVALID_MODIFIER_ON_SETTER` no `async` (`d49`), `external` com corpo ≠ `;` →
`EXTERNAL_METHOD_WITH_BODY` **no `external`** (`pi:3891`, `c147`), `parseFunctionBody(allowAbstract =
isExternal)`.

`parseGetterOrFormalParameters(token, name, isGetter, kind)` (`pi:1541`):

| situação | efeito |
|---|---|
| `(` e é getter | `GETTER_WITH_PARAMETERS` no `(`; lê os parâmetros (`d48`) |
| `(` | `parseFormalParameters` |
| sem `(`, getter | `handleNoFormalParameters` |
| sem `(`, não getter | `missingParameterMessage(kind)` **no nome** (em `operator +`, no operador; em `operator unary-`, no `-`); `insertParens` sintéticos; lista vazia |

`missingParameterMessage` (`pi:1842`): `MemberKind.StaticMethod`/`NonStaticMethod` →
`MissingMethodParameters`; `FunctionTypeAlias` → `MissingTypedefParameters`; **todos os outros**
(topo, extension, extension type, factory, parâmetro-função, local) → `MissingFunctionParameters`.
O `kind` de `parseMethod` (`pi:4942-4958`): `TopLevel/Class/Mixin/Enum` → `(Non)StaticMethod`;
`Extension` → `Extension(Non)StaticMethod`; `ExtensionType` → `ExtensionType(Non)StaticMethod`. Logo
método sem `(` em **extension e extension type** é `missing_function_parameters`.
`parseFormalParametersRequiredOpt` (`pi:1755`; factory e parâmetro-função) relata no token
**seguinte**, não no nome (`c129`).

`parseFunctionBody(token, ofFunctionExpression, allowAbstract)` (`pi:5415`), `next = token.next`:

| `next` | efeito |
|---|---|
| `native` | `parseNativeClause`; depois `;` → corpo nativo; senão `EXTERNAL_METHOD_WITH_BODY` em next e segue |
| `;` | se `!allowAbstract`: `MISSING_FUNCTION_BODY` **no `;`**; corpo vazio |
| `=>` | `parseExpressionFunctionBody` (+ `ensureSemicolon`) |
| `=` | `MISSING_FUNCTION_BODY` no `=`; insere `=>` sintético; lê a expressão; `ensureSemicolon` (`c132`) |
| `return` | `MISSING_FUNCTION_BODY` no `return`; insere `=>`; lê como corpo de expressão (`c133`) |
| palavra/identificador seguido de `=>` | `UNEXPECTED_TOKEN` nela; lê o `=>` (`c135`) |
| palavra/identificador seguido de `{` | `UNEXPECTED_TOKEN` nela; lê o bloco (`c134`) |
| `{` | bloco; comando sem progresso → `UNEXPECTED_TOKEN` no token e pula um (`pi:5490`) |
| outro (inclusive EOF, `}`, `.`) | `ensureBlock(BlockKind.functionBody)`: `MISSING_FUNCTION_BODY` **em next**, bloco sintético, nada consumido (`c08`, `c28`, `d43`) |

`allowAbstract` por chamador: topo = `externalToken != null` (`pi:3896`); membro = `(staticToken ==
null || externalToken != null) && inPlainSync` (`pi:4985`); factory = `true` se `external`, senão
`false` (`pi:5158/5169`); função local e expressão de função = `false`. Com `async;` num método de
instância, `inPlainSync` é falso → `missing_function_body` no `;` (`d60`); o analyzer ainda relata
`concrete_class_with_abstract_member` porque a árvore fica com corpo vazio.

`parseFields` (`pi:3655`) — a ordem dos relatos é a do código:
1. `covariant` + `final` sem `late` → `FINAL_AND_COVARIANT` **no `covariant`** (`c67`).
2. sem tipo e sem `var/final/const` → `MISSING_CONST_FINAL_VAR_OR_TYPE` **no nome**; com tipo e `var` →
   `VAR_AND_TYPE` no `var` (`d47`).
3. `abstract` + `external` → `ABSTRACT_EXTERNAL_FIELD` no `abstract` (`c179`).
4. nome (`ensureIdentifier`: `MISSING_IDENTIFIER`/`EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD`).
5. `covariant late final x = e` → `FINAL_AND_COVARIANT_LATE_WITH_INITIALIZER` no `covariant`.
6. por variável (`parseFieldInitializerOpt`, `pi:3922`): nome igual ao da classe →
   `MEMBER_WITH_CLASS_NAME`; sem `=` e nome não sintético: `const` → `CONST_NOT_INITIALIZED`; no
   **topo**, `final` sem `late`/`abstract`/`external` → `FINAL_NOT_INITIALIZED` (em classe quem relata
   é o `ErrorVerifier`).
7. `;` que falta → `ensureSemicolon` (`pi:4293`): `EXPECTED_TOKEN ';'` no **token anterior** (o último
   lido, `findPreviousNonZeroLengthToken`), `;` sintético.
8. por tipo de declaração: extension sem `static`/`external` → `EXTENSION_DECLARES_INSTANCE_FIELD` no
   1º nome; extension type idem com o modelo `ExtensionTypeDeclaresInstanceField`, que o conversor
   **descarta** (`error_converter.dart:529`: relatado pelo `ErrorVerifier`).
Depois, o AstBuilder (`endClassFields`, `ab:1241`): `abstract static` → `ABSTRACT_STATIC_FIELD`,
`abstract late` → `ABSTRACT_LATE_FIELD`, `external late` → `EXTERNAL_LATE_FIELD`, todos no
`abstract`/`external` (`c156`, `d105`).

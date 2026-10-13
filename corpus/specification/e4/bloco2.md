##### Duplicatas idênticas somem

O `RecordingErrorListener` guarda os erros num `Set<AnalysisError>` (`analyzer/lib/error/listener.dart:432`,
`:452`) e a igualdade é por `errorCode` (identidade), `offset`, `length`, `message` e `source`
(`analyzer/lib/error/error.dart:236-249`). Duas chamadas do parser com o mesmo código no mesmo token viram
**um** diagnóstico (`for () {}` pede `MISSING_IDENTIFIER` duas vezes no `)` e `EXPECTED_TOKEN ';'` duas vezes no
`(`; sai um de cada). `ParserErrorCode.EXPECTED_TOKEN` e `ScannerErrorCode.EXPECTED_TOKEN` são objetos
distintos: os dois saem (o `a[1; }` acima).

#### E.4.2 Comandos

##### `parseStatement` / `parseStatementX` (`pi:5577`, `pi:5589`)

`parseStatement` só conta profundidade (`statementDepth++ > 500` → `recoverFromStackOverflow`, `pi:5578-5583`,
`STACK_OVERFLOW`). `parseStatementX` decide pelo **primeiro token** (`next = token.next`):

| `next` | função | linha |
|---|---|---|
| `kind == IDENTIFIER_TOKEN` (identificador puro) seguido de `:` | `parseLabeledStatement` | `pi:5590-5593` |
| identificador puro (outro) | `parseExpressionStatementOrDeclarationAfterModifiers(token, token, null, null, null)` — direto, sem olhar modificadores | `pi:5594-5599` |
| `{` | se `allowPatterns` e o token depois do `}` casado é `=`: `parseExpressionStatement` (atribuição por padrão de mapa); senão `parseBlock(token, BlockKind.statement)` | `pi:5602-5609` |
| `return` | `parseReturnStatement` | `pi:5610` |
| `var` / `final` | seguinte **não** é modificador: `…AfterModifiers(varOrFinal, token, null, varOrFinal, null)`; senão `parseExpressionStatementOrDeclaration` (recuperação de modificadores) | `pi:5612-5618` |
| `if` | `parseIfStatement` | `pi:5619` |
| `await` seguido de `for` | `parseForStatement(token.next, awaitToken)` | `pi:5621-5623` |
| `for` | `parseForStatement(token, null)` | `pi:5624` |
| `rethrow` / `while` / `do` / `try` / `switch` / `break` / `continue` / `assert` / `;` | `parseRethrowStatement` / `parseWhileStatement` / `parseDoWhileStatement` / `parseTryStatement` / `parseSwitchStatement` / `parseBreakStatement` / `parseContinueStatement` / `parseAssertStatement` / `parseEmptyStatement` | `pi:5626-5643` |
| `yield` | `asyncState == Sync`: rótulo se `yield :`; `parseYieldStatement` se `looksLikeYieldStatement`; senão declaração/expressão. `SyncStar`/`AsyncStar`/`Async`: sempre `parseYieldStatement` | `pi:5644-5664` |
| `const` | `parseExpressionStatementOrConstDeclaration` | `pi:5665` |
| `await` (sem `for`) | em função síncrona e **não** `looksLikeAwaitExpression`: declaração/expressão (`await` é nome); senão `parseExpressionStatement` (o `parseAwaitExpression` relata `AWAIT_IN_WRONG_CONTEXT`) | `pi:5667-5677` |
| `set` seguido de identificador | `UNEXPECTED_TOKEN` no `set` e recomeça em `parseStatementX(token.next)` | `pi:5678-5682` |
| outro `isIdentifier` (embutido/pseudo) seguido de `:` | `parseLabeledStatement` | `pi:5683-5686` |
| qualquer outra coisa (palavras reservadas sem comando próprio, operadores, literais, `(`, `[`, `@`, `late`, `void`, `)`, `]`, `}`…) | `parseExpressionStatementOrDeclaration` | `pi:5687-5690` |

Não existe "comando inválido" no parser: o que não é comando vira **comando de expressão**, e a expressão
vazia vira identificador sintético. `case`, `default`, `else`, `catch`, `finally`, `in`, `class`, `enum`,
`extends`, `with`, `)`, `]` não têm tratamento em `parseStatementX`.

##### Sem progresso → `UNEXPECTED_TOKEN` (três laços, lexemas diferentes)

Quando `parseStatement` devolve um token cujo `next` ainda é o token de partida (a expressão virou
identificador sintético + `;` sintético inseridos **antes** do token ruim), o laço relata e pula um token:

| laço | código | lexema da mensagem | posição |
|---|---|---|---|
| corpo de função, `parseFunctionBody` `pi:5487-5497` | `reportRecoverableError(token, templateUnexpectedToken.withArguments(token)); token = token.next` — `token` é o `;` **sintético** devolvido | **`';'`** | o token ruim (`findNonZeroLengthToken` anda do `;` sintético para ele) |
| bloco, `parseBlock` `pi:8629-8640` | `token = token.next; reportRecoverableError(token, …withArguments(token))` | o do token ruim | o token ruim |
| corpo de `case`, `parseStatementsInSwitchCase` `pi:9087-9096` | `reportRecoverableError(next, …withArguments(next)); token = next` | o do token ruim | o token ruim |

O comando de expressão **não consome** o token (e portanto cai aqui) quando `ExpressionIdentifierContext.ensureIdentifier`
(`ic:324-377`, tabela em §E.4.3) insere o sintético antes dele: operadores, `. , ( ) [ ] { } ? : ;`, EOF,
`as`/`is` e as palavras de `looksLikeStatementStart` (`fe:parser/identifier_context.dart:391-395`: `@ assert break
continue do else final for if return switch try var void while`). Dos que chegam ao comando de expressão sem
virar outra coisa, dão a trinca completa: `)`, `]`, `:` e `else` (medidos) e `,` (não verificado). A ordem de emissão é
`MISSING_IDENTIFIER` (no token) → `EXPECTED_TOKEN ';'` (no token real **anterior**) → `UNEXPECTED_TOKEN` (no token).

```dart
f() { ) }
```
- `expected_token @4 len 1 (1:5) Expected to find ';'.` (no `{`)
- `missing_identifier @6 len 1 (1:7) Expected an identifier.`
- `unexpected_token @6 len 1 (1:7) Unexpected text ';'.` (lexema do `;` sintético)

```dart
f() { { ) } }
```
- `expected_token @6 len 1 (1:7)`, `missing_identifier @8 len 1 (1:9)`,
  `unexpected_token @8 len 1 (1:9) Unexpected text ')'.` (bloco: lexema real)

```dart
f() { else }
```
- `expected_token @4 len 1 (1:5) Expected to find ';'.`, `missing_identifier @6 len 4 (1:7)`,
  `unexpected_token @6 len 4 (1:7) Unexpected text ';'.`; dentro de bloco (`f() { { else } }`): `Unexpected text 'else'.` @8.

Outros medidos: `f() { ] }` igual ao `)`; `f() { 1 ? 2 : 3 : 4; }` → `expected_token @14 ';'` (no `3`),
`missing_identifier @16`, `unexpected_token @16 Unexpected text ';'.` (no segundo `:`);
`f(x) { switch (x) { case 1: ) } }` → `expected_token @26` (no `:`), `missing_identifier @28`,
`unexpected_token @28 Unexpected text ')'.`.

##### `MISSING_STATEMENT` (quem relata é o AstBuilder)

O parser **nunca** emite `ExpectedStatement` (a constante `messageExpectedStatement` não aparece em
`parser_impl.dart`). O `AstBuilder.handleExpressionStatement` (`ab:4238-4264`) relata quando a expressão do
comando é um `SimpleIdentifier` cujo token tem `keyword != null` e `isBuiltInOrPseudo == false`
(`fe:scanner/token.dart:446`) — isto é, **uma palavra reservada que o parser aceitou como identificador**
(com `EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD`, `ic:359-361`) e que ficou **sozinha** como comando. Detalhes na
ficha do código (§E.4.6).

##### Declaração local × expressão (`pi:7968`, `pi:8011`, `pi:8064`)

```
parseExpressionStatementOrDeclaration(start, [forPartsContext]):           pi:8011
  se next == '@': parseMetadataStar
  se isModifier(next):
     'augment' + 'super'                → parseExpressionStatement(start)      pi:8024
     var/final/const                    → varFinalOrConst = next
     late (+ var/final)                 → lateToken (, varFinalOrConst)
     ainda modificador (static, var var…) → ModifierContext.parseVariableDeclarationModifiers
                                          (DUPLICATED_MODIFIER, EXTRANEOUS_MODIFIER, MODIFIER_OUT_OF_ORDER)
  → …AfterModifiers(token, start, lateToken, varFinalOrConst, null, forPartsContext)

…AfterModifiers(beforeType, start, late, vfc, typeInfo, [forPartsContext]): pi:8064
  1. `late` em biblioteca sem NNBD → UnexpectedModifierInNonNnbd (não verificado no oráculo)      pi:8068-8083
  2. allowPatterns && vfc ∈ {var, final} && skipOuterPattern(beforeType) != null
       && depois vem '=' (ou 'in', se forPartsContext)                                        pi:8085-8113
       → late: LATE_PATTERN_VARIABLE_DECLARATION no late
       → for: forPartsContext.patternKeyword = vfc; return parsePattern(…, declaration)
       → senão parsePatternVariableDeclarationStatement
  3. typeInfo ??= computeType(beforeType, required: false); token = typeInfo.skipType(…)      pi:8115-8118
  4. forPartsContext != null: `late` → EXTRANEOUS_MODIFIER                                    pi:8120-8124
     senão, se looksLikeLocalFunction(next):                                                  pi:8126-8152
        vfc → EXTRANEOUS_MODIFIER no vfc; senão late → EXTRANEOUS_MODIFIER no late
        → parseNamedFunctionRest(…, isFunctionExpression: false)
  5. beforeType == start && typeInfo.isNullable && typeInfo.couldBeExpression (`a ? b …`):    pi:8155-8202
        se !looksLikeName(next): MISSING_IDENTIFIER em next + identificador sintético
        se depois do nome vem '=': analisa a expressão em seco (NullListener + rewriter desfazível);
             se depois dela vem ':' → é condicional: typeInfo = noType, token = start
        senão, se o token depois do nome não é palavra-chave nem `; , )` nem EOF → condicional
  6. token == start (nenhuma anotação, modificador ou tipo):                                  pi:8204-8212
        for: return start;   senão: return parseExpressionStatement(start)
  7. next.type.isBuiltIn && beforeType == start && couldBeExpression && next ∈ {as, is}
       && o token depois não é `=`, `;`, `,` → expressão (`a as B;` contra `a as;`)           pi:8214-8233
  8. next.isIdentifier:                                                                       pi:8235-8248
        sem vfc e typeInfo == noType → MISSING_CONST_FINAL_VAR_OR_TYPE em next
        vfc == var e typeInfo != noType → VAR_AND_TYPE no `var`
  9. typeInfo.parseType; beginVariablesDeclaration;                                           pi:8256-8263
     for: devolve o token antes do nome (o chamador lê as variáveis);
     senão parseVariablesDeclarationRest(token, endWithSemicolon: true)
```

- **Papel de `computeType`** (`fe:parser/type_info.dart:159`): é ele que decide tudo no passo 3 — só devolve
  tipo se o que vem **depois** "parece nome" (`looksLikeName`), senão `noType` e o passo 6 lê expressão. Por
  isso `x y z;` é a declaração `x y` + `;` que falta + expressão `z` (`undefined_class @6`, `expected_token @8 ';'`,
  `undefined_identifier @10`), `int class = 10;` é a expressão `int` + `;` que falta + `class = 10` (§E.4.6), e
  `a xor b;` é a declaração `a xor` (a recuperação `xor` → `^` só existe no laço de precedência, §E.4.3).
- **`looksLikeLocalFunction`** (`pi:7936-7958`): `next` é identificador, opcionalmente `<…>` válido
  (`computeTypeParamOrArg != noTypeParamOrArg`), depois `(`…`)` casado seguido de `{`, `=>`, `async` ou
  `sync`; ou, recuperação, identificador seguido direto de `=>`. Não exige tipo de retorno: `a(){}` em
  comando é função local `a`.
- **`const`** (`parseExpressionStatementOrConstDeclaration`, `pi:7968-7992`): com `computeType == noType`, é
  declaração só se depois do `const` vem identificador seguido de `=`, palavra/identificador, `;`, `,` ou `{`;
  senão expressão (`const A();`). `const A;` é declaração (`const_not_initialized @12`, medido).
- **`onlyParseVariableDeclarationStart`** do texto antigo é, no 3.6.2, o parâmetro `forPartsContext != null`
  (comentário em `pi:8405-8408`): só anotações, modificadores e tipo são lidos; devolve `start` se não é
  declaração (passos 6 e 7).

Medidos: `f() { var int x = 1; }` → `var_and_type @6 len 3`; `f() { var var x; }` → `duplicated_modifier @10 len 3`;
`f() { late int f() => 1; var g() {} }` → `extraneous_modifier @6 len 4 'late'`, `extraneous_modifier @25 len 3 'var'`;
`f(a, b) { a ? b; }` → declaração `a? b` (`not_a_type @10`, sem erro sintático);
`f() { int? x = 1 : 2; }` → condicional (`non_bool_condition @6 len 3`); `f() { var; }` →
`missing_identifier @9 len 1` (no `;`); `f() { final x }` → `expected_token @12 len 1 ';'` (no `x`);
`f() { int x y; }` → `expected_token @10 ';'` (no `x`) e `y` vira comando de expressão;
`f() { var x = 1 var y = 2; }` → `expected_token @14 len 1 ';'` (no `1`).

##### Rótulos (`pi:5750`, `pi:5763`)

`parseLabeledStatement`: `label+` (`parseLabel` = `ensureIdentifier(labelDeclaration)` + `:`), depois
`parseStatement`. Só entra por `parseStatementX` quando o segundo token é `:`; por isso o contexto
`labelDeclaration` (`ic:685-718`) praticamente não recupera. Rótulo antes de `}`: o comando rotulado é o
comando de expressão vazio — `f() { L: }` → `expected_token @7 len 1 ';'` (no `:`), `missing_identifier @9 len 1`
(no `}`), sem `UNEXPECTED_TOKEN` (houve progresso: o rótulo foi consumido). Em `switch`, rótulos antes de
`case`/`default` são do caso (`peekPastLabels`, `pi:9060`); rótulo solto antes do `}` do switch é "erro de
comando" (comentário em `pi:9084`): `switch (x) { case 1: L: }` → `expected_token @29 ';'`, `missing_identifier @31`.

##### `LoopState` — `break`/`continue` (`fe:parser/loop_state.dart`, `pi:302`, `pi:377-381`)

Um único campo `loopState ∈ {OutsideLoop, InsideSwitch, InsideLoop}`, salvo e restaurado por construção:

| construção | efeito | linha |
|---|---|---|
| início | `OutsideLoop` | `pi:302` |
| corpo de função **em bloco** (`parseFunctionBody`; qualquer função: topo, método, local, expressão de função) | salva; `OutsideLoop`; restaura no `}` | `pi:5483-5484`, `pi:5502` |
| corpo `=>` | não mexe (não há comandos ali, a não ser dentro de outro bloco de função) | `pi:5506-5521` |
| corpo de `for`, `for-in`, `while`, `do` (só o corpo; as partes do cabeçalho e a condição do `do…while` ficam fora) | salva; `InsideLoop`; restaura | `pi:8466-8469`, `pi:8527-8530`, `pi:8583-8586`, `pi:8602-8605` |
| `switch` **comando** (o bloco inteiro) | salva; `InsideSwitch` **só se** estava `OutsideLoop` (dentro de laço continua `InsideLoop`); restaura | `pi:8961-8966` |
| `switch` **expressão**, `if`, `try`, bloco, rótulo | não mexem | — |

| comando | condição do erro | código | linha |
|---|---|---|---|
| `break;` | `loopState == OutsideLoop` | `BREAK_OUTSIDE_OF_LOOP` | `pi:9118-9119` |
| `break L;` | nunca (o parser não confere nada com rótulo; bloco rotulado é alvo válido) | — | `pi:9115-9117` |
| `continue;` | `loopState == InsideSwitch` | `CONTINUE_WITHOUT_LABEL_IN_CASE` | `pi:9215-9220` |
| `continue;` | `loopState == OutsideLoop` | `CONTINUE_OUTSIDE_OF_LOOP` | `pi:9215-9220` |
| `continue L;` | `loopState == OutsideLoop` | `CONTINUE_OUTSIDE_OF_LOOP` | `pi:9211-9213` |

Posição: sempre a palavra `break`/`continue` (len 5 / 8). O comando é montado normalmente depois
(`handleBreakStatement`/`handleContinueStatement`, `ab:3792`, `ab:3979`) e a resolução ainda pode acrescentar
`LABEL_UNDEFINED` / `CONTINUE_LABEL_INVALID`.

##### `for` (`pi:8335-8570`)

```
parseForStatement(token, awaitToken):                                       pi:8335
  ctx = ForPartsContext()
  token = parseForLoopPartsStart(awaitToken, forToken, ctx)                 pi:8373
     sem '(' → EXPECTED_TOKEN '(' no token seguinte; insere `( ; ; ) <id> ;` sintéticos
               (com await: `( <id> in <id> ) <id> ;`)                       pi:8376-8403
     → parseExpressionStatementOrDeclaration('(', ctx)   (só anotações/modificadores/tipo, ou padrão)
  se ctx.patternKeyword != null:                                            pi:8343-8358
     '=' → parseExpression; handleForInitializerPatternVariableAssignment; parseForRest
     senão ('in' garantido pelo passo 2 acima) → parseForInRest(…, patternKeyword, identifier: null)
  identifier = token.next
  token = parseForLoopPartsMid(token, awaitToken, forToken)                 pi:8415
     leu declaração (token != '(') → parseVariablesDeclarationRest(token, endWithSemicolon: false)
     senão ';' → inicializador vazio
     senão → parseExpression
     depois: ';' com await → INVALID_AWAIT_IN_FOR no `await`                pi:8434-8437
             ':' → COLON_IN_PLACE_OF_IN no ':'                              pi:8440-8441
             nem 'in' nem ';' e com await → EXPECTED_TOKEN 'in' + `in` sintético   pi:8442-8447
  'in' ou ':' → parseForInRest(…, identifier)   senão parseForRest          pi:8361-8368

parseForLoopPartsRest:                                                      pi:8475
  ensureSemicolon(token)                         (1º ';')
  ';' → condição vazia; senão parseExpressionStatement (condição + 2º ';' com ensureSemicolon)
  laço: ')' para; senão parseExpression e continua enquanto ','
  se parou em token ≠ ')' casado → UNEXPECTED_TOKEN nele e pula para o ')' casado   pi:8500-8503

parseForInLoopPartsRest:                                                    pi:8536
  await e !inAsync → ASYNC_FOR_IN_WRONG_CONTEXT no `await`                  pi:8543-8545
  identifier != null:
     !identifier.isIdentifier → MISSING_IDENTIFIER no identifier            pi:8548-8552
     identifier != token (há algo entre o nome e o `in`):
        identifier.next == '=' → INITIALIZED_VARIABLE_IN_FOR_EACH no '='    pi:8554-8556
        senão → UNEXPECTED_TOKEN em identifier.next                         pi:8557-8560
  parseExpression(in); ensureCloseParen
```

| entrada (oráculo vivo) | diagnósticos sintáticos |
|---|---|
| `for (var (a, b) = (1, 2); a < 3; a++) {}` | nenhum (válido: `ForPartsWithPattern`) |
| `for (var (_,) = (0,);;) {}` | nenhum |
| `for (final (a, b) in e) {}` | nenhum |
| `for ((a, b) in e) {}` | `missing_identifier @12 len 1` (no `(`: `identifier` não é identificador) |
| `for (this.x in y) {}` | `missing_identifier @12 len 4` (no `this`) |
| `for (x in y) {}` | nenhum |
| `for (var x, z in y) {}` | `unexpected_token @17 len 1 Unexpected text ','.` |
| `for (var x = 1 in y) {}` | `initialized_variable_in_for_each @18 len 1` (no `=`) |
| `for (var x : y) {}` | `colon_in_place_of_in @18 len 1` |
| `for (var x of y) {}` | `var_and_type @12 len 3`; `expected_token @18 len 2 ';'` (no `of`); `expected_token @21 len 1 ';'` (no `y`) — é o `for` clássico `var x of; y; )` |
| `for (var x in y z) {}` | `expected_token @26 len 1 ')'` (no `z`; pula até o `)` casado) |
| `for (var x in) {}` | `missing_identifier @19 len 1` (no `)`) |
| `for (var i = 0; i < 3) {}` | `expected_token @26 len 1 ';'` (no `3`) |
| `for (var i = 0; i < 3; i++; i--) {}` | `unexpected_token @32 len 1 Unexpected text ';'.` |
| `for () {}` | `expected_token @10 len 1 ';'` (no `(`); `missing_identifier @11 len 1` (no `)`) |
| `for {}` | `expected_token @10 len 1 '('` (no `{`); mais nada (o `{}` vira o corpo depois do cabeçalho sintético; `dead_code @10 len 2`) |
| `for (;;) }` | `expected_token @13 len 1 ';'` (no `)`); `missing_identifier @15 len 1` (no `}`) |
| `for (late var x in y) {}` | `extraneous_modifier @12 len 4` |
| `await for (var x in y) {}` em função síncrona | `async_for_in_wrong_context @7 len 5` |
| `await for (;;) {}` em `async` | `invalid_await_in_for @13 len 5` |
| `await for () {}` em `async` | `missing_identifier @24 len 1` e `expected_token @24 len 1 'in'` (ambos no `)`) |

##### `if`, `while`, `do` (`pi:8297`, `pi:8577`, `pi:8597`, `pi:6729`)

`ensureParenthesizedCondition`: sem `(` → `ExpectedToken '('` no token seguinte + `()` sintéticos
(`rewriter.insertParens(token, includeIdentifier: false)`, `pi:6731-6736`), e a condição é lida **dentro** do par
sintético vazio: identificador que falta no primeiro token real. `if`: `allowCase: allowPatterns` — o
`case` depois da condição só é lido com padrões ligados (`pi:6838`). `do`: sem `while` →
`ExpectedButGot 'while'` + `while` sintético (`pi:8608-8612`), depois a condição e `ensureSemicolon`.

| entrada | diagnósticos |
|---|---|
| `f(x) { if x {} }` | `expected_token @10 '('`, `missing_identifier @10`, `expected_token @10 ';'` — os três no `x` (len 1): `()` vazio, depois `x` e `{}` são comandos |
| `f() { if (true) }` | `expected_token @14 ';'` (no `)`), `missing_identifier @16` (no `}`) |
| `f() { if (true) else ; }` | `expected_token @14 ';'`, `missing_identifier @16 len 4` (no `else`); o `else` é consumido como o do `if` (sem `UNEXPECTED_TOKEN`) |
| `f() { if (true) { } else }` | `expected_token @20 len 4 ';'` (no `else`), `missing_identifier @25` |
| `f(x) { if (x }` | `expected_token @11 ';'` (no `x`), `missing_identifier @13`, `expected_token @13 ')'` (scanner, no `}`) |
| `f() { if (` (EOF) | `expected_token @9 ';'`, `missing_identifier @10 len 0`, `expected_token @10 len 1 ')'` e `'}'` (scanner) |
| `f() { while () {} }` | `missing_identifier @13 len 1` |
| `f() { do {} }` | `expected_token @10 ';'` (no `}` do corpo), `expected_token @12 'while'`, `expected_token @12 '('`, `missing_identifier @12` (os três no `}`) |
| `f() { do {} while (true) }` | `expected_token @23 len 1 ';'` (no `)`) |
| `f(x) { if (x case var a = 1) {} }` | `expected_token @24 len 1 ')'` (no `=`) |

##### `switch` (`pi:8956-9104`)

```
parseSwitchStatement:  ensureParenthesizedCondition(allowCase: false); LoopState; parseSwitchBlock
parseSwitchBlock(token):                                                    pi:8976
  beginSwitch = ensureBlock(token, BlockKind.switchStatement)
       sem '{' → EXPECTED_SWITCH_STATEMENT_BODY no token ANTERIOR (o ')'), bloco `{}` sintético     pi:4200-4217
  enquanto next ∉ {'}', EOF}:
     peek = peekPastLabels(beginCase)            (pula `id :` repetidos)
     laço:
       peek == 'default':  parseLabel*;  já havia default → SWITCH_HAS_MULTIPLE_DEFAULT_CASES no `default`
                           ensureColon; break                                               pi:8990-9002
       peek == 'case':     parseLabel*;  já havia default → SWITCH_HAS_CASE_AFTER_DEFAULT_CASE no `case`
                           allowPatterns ? parsePattern(case, matching) : parseExpression(case)
                           'when' → parseExpression (guarda)
                           ensureColon; expressionCount++; peek = peekPastLabels(next)      pi:9003-9032
       expressionCount > 0: break
       senão (nem case nem default):  EXPECTED_TOKEN 'case' em **peek** (depois dos rótulos);
                           pula TUDO até o '}' do switch; break                             pi:9035-9045
     parseStatementsInSwitchCase(…)                                                         pi:9068
       até peek ∈ {case, default} ou ('}' logo em seguida): parseStatement (+ sem progresso)
```

- `ensureColon` (`pi:4257-4263`): `ExpectedButGot ':'` no token seguinte + `:` sintético; nada é pulado.
- `defaultKeyword` nunca é zerado: depois do primeiro `default`, **todo** `case` posterior leva o erro.
- Corpo vazio de `case` é aceito pelo parser. Comandos depois de `break` no mesmo `case` também.
- `case`/`default` **fora** de `switch` não têm tratamento: são comando de expressão (§E.4.6 `missing_statement`).

| entrada | diagnósticos |
|---|---|
| `switch (x) { default: case 1: }` | `switch_has_case_after_default_case @29 len 4` |
| `switch (x) { case 1: break; default: break; case 2: break; default: }` | `switch_has_case_after_default_case @51 len 4`; `switch_has_multiple_default_cases @66 len 7` |
| `switch (x) { foo: }` | `expected_token @25 len 1 'case'` (no `}`: depois do rótulo) |
| `switch (x) { 1: break; }` | `expected_token @20 len 1 'case'` (no `1`); o resto do bloco é pulado (sem `break_outside_of_loop`) |
| `switch (x) { case 1: case 2 }` | `expected_token @35 len 1 ':'` (no `}`) |
| `switch (x) { default }` | `expected_token @28 len 1 ':'` |
| `switch (x) { case: break; }` | `missing_identifier @24 len 1` (no `:`; padrão constante vazio) |
| `switch (x) }` | `expected_body @16 len 1` (no `)`) |
| `switch x { }` | `expected_token @14 '('`, `missing_identifier @14`, `expected_body @14`, `expected_token @14 ';'` — todos no `x`; `x` e `{ }` viram comandos |
| `switch (x) L: { case 1: break; }` | ver `missing_statement` (§E.4.6) |
| `switch (x) { L: case 1: break; M: default: break; }` | nenhum |

##### `try` / `catch` / `on` (`pi:8819-8949`)

`parseBlock(tryKeyword, BlockKind.tryStatement)`; enquanto `on` ou `catch`: `on` → `computeType(required: true)
.ensureTypeNotVoid`; `catch` → sem `(`: `CATCH_SYNTAX` no token seguinte + `( <id> )` sintéticos (`pi:8854-8858`);
primeiro parâmetro não identificador → `IdentifierContext.catchParameter`; depois do nome nem `)` nem `,`:
`CATCH_SYNTAX` nesse token (`pi:8870-8874`); depois do segundo nome sem `)`: `CATCH_SYNTAX_EXTRA_PARAMETERS`
no token seguinte (`pi:8912-8917`); corpo `parseBlock(…, BlockKind.catchClause)`. `finally` → bloco. Sem `catch`,
`on` nem `finally`: `MISSING_CATCH_OR_FINALLY` **no `try`** (`pi:8942-8944`). Os blocos que faltam relatam
no token **anterior** (`ensureBlock` com `message`, `pi:4211-4212`). `on` depois do bloco do `try` é sempre
cláusula (comentário `pi:8833-8835`). `catch`/`finally` soltos são comando de expressão.

| entrada | diagnósticos |
|---|---|
| `f() { try {} }` | `missing_catch_or_finally @6 len 3` |
| `f() { try {} catch {} }` | `catch_syntax @19 len 1` (no `{`) |
| `f() { try {} catch (e, s, t) {} }` | `catch_syntax_extra_parameters @24 len 1` (na `,`) |
| `f() { try {} on {} }` | `expected_type_name @16 len 1` |
| `f() { try {} catch (e) }` | `expected_body @21 len 1` (no `)`) |
| `f() { catch (e) {} }` | `expected_identifier_but_got_keyword @6 len 5`; `expected_token @14 len 1 ';'` (no `)`): é a chamada `catch(e)` + bloco |
| `f() { finally {} }` | `expected_identifier_but_got_keyword @6 len 7`, `expected_token @6 ';'`, `missing_statement @6 len 7` |

##### `assert`, `return`, `yield`, `throw` (`pi:9131`, `pi:5726`, `pi:5698`, `pi:8766`)

- `parseAssert(token, kind)`: sem `(` → `ExpectedButGot '('` no token seguinte + `( <id> )` sintéticos; condição;
  `,` mensagem `,`? ; se o token seguinte não é o `)` casado: fecho sintético → move; senão
  `UNEXPECTED_TOKEN` nesse token e pula até o `)` (`pi:9162-9175`). `Assert.Expression` → `AssertAsExpression`
  (descartado, §E.4.1); `Assert.Statement` → `ensureSemicolon`.
- `parseReturnStatement`: `return ;` ou `return e ;`; em gerador com valor, `handleInvalidStatement(begin,
  GeneratorReturnsValue)` → `RETURN_IN_GENERATOR` no `return` (`pi:5738-5741`).
- `parseYieldStatement`: fora de gerador, `YieldNotGenerator` no `yield` (`pi:5711-5712`) → `YIELD_IN_NON_GENERATOR`
  (o `ErrorVerifier` relata outro, com o comando inteiro: dois diagnósticos de lengths diferentes).
- `throw` sem expressão: `MISSING_EXPRESSION_IN_THROW` (`pi:8773`).

| entrada | diagnósticos |
|---|---|
| `f() { assert; }` | `expected_token @12 len 1 '('` (no `;`) |
| `f() { assert(true, 'm', 3); }` | `unexpected_token @24 len 1 Unexpected text '3'.` |
| `f() { var x = assert(true); }` | só `undefined_identifier @14 len 6` (nenhum sintático) |
| `f() { return }` | `expected_token @6 len 6 ';'` (no `return`), `missing_identifier @13 len 1` (no `}`) |
| `f() { return 1 2; }` | `expected_token @13 len 1 ';'` (no `1`) |
| `f() { return return; }` | `unexpected_token @13 len 6 'return'`, `missing_identifier @19 len 1` |
| `f() sync* { return 1; }` | `return_in_generator @12 len 6` |
| `f() { yield 1; }` (síncrona) | `expected_token @6 len 5 ';'` (no `yield`): `yield` é identificador, `1` outro comando |
| `f() async { yield x; }` | `yield_in_non_generator @27 len 5` (parser) e `@27 len 8` (verificador) |
| `f() { throw; }` | `missing_expression_in_throw @11 len 1` (no `;`) |
| `f() => 1` (EOF) | `expected_token @7 len 1 ';'` (no `1`) |


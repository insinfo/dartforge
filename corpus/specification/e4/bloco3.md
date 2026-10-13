#### E.4.3 Expressões

##### Precedência (`fe:scanner/token.dart:15-32` e `TokenType.precedence`)

| nível | nome | tokens |
|---:|---|---|
| 1 | `ASSIGNMENT` | `=` `+=` `-=` `*=` `/=` `~/=` `%=` `<<=` `>>=` `>>>=` `&=` `^=` `|=` `??=` (e `&&=`/`||=` do scanner) |
| 2 | `CASCADE` | `..` `?..` |
| 3 | `CONDITIONAL` | `?` (vira 17 em padrão, ou em expressão diante de `[` quando `canParseAsConditional` falha, `pi:6276-6289`) |
| 4 | `IF_NULL` | `??` |
| 5 / 6 | `LOGICAL_OR` / `LOGICAL_AND` | `||` / `&&` |
| 7 | `EQUALITY` | `==` `!=` `===` `!==` |
| 8 | `RELATIONAL` | `<` `>` `<=` `>=` `is` `as` |
| 9 / 10 / 11 | `BITWISE_OR` / `XOR` / `AND` | `|` / `^` / `&` |
| 12 | `SHIFT` | `<<` `>>` `>>>` |
| 13 | `ADDITIVE` | `+` `-` |
| 14 | `MULTIPLICATIVE` | `*` `/` `%` `~/` |
| 15 | `PREFIX` | `!` `~` (o `!` pós-fixo é reclassificado por `_computePrecedence`, `pi:6256-6267`: 17 se seguido de `.` `?` `(` `[` `?.`, senão 16) |
| 16 | `POSTFIX` | `++` `--` `!` |
| 17 | `SELECTOR` | `.` `?.` `(` `[` `[]` |

##### `parseExpression` e o laço (`pi:5802`, `pi:5889`, `pi:5921`)

```
parseExpression(token):                                                      pi:5802
  expressionDepth++ > 500 → STACK_OVERFLOW no próximo token; pula até o fecho do grupo
        (ou até `) ] } ;`) e insere identificador sintético                  pi:5803-5826
  allowPatterns && looksLikeOuterPatternEquals(token) → parsePatternAssignment          pi:5828
  next == 'throw' → parseThrowExpression(allowCascades: true)
  senão parsePrecedenceExpression(token, ASSIGNMENT, allowCascades: true, ConstantPatternContext.none)

parsePrecedenceExpression(token, precedence, allowCascades, cpc):            pi:5889
  token = parseUnaryExpression(token, allowCascades, cpc)
  typeArg = computeMethodTypeArguments(depois de um `!` opcional)
  typeArg existe: lê os argumentos de tipo; se não vem '(' → handleTypeArgumentApplication
        (em padrão constante: INVALID_CONSTANT_PATTERN_GENERIC no '<')        pi:5899-5915
  → _parsePrecedenceExpressionLoop

_parsePrecedenceExpressionLoop(precedence, allowCascades, typeArg, token, cpc):   pi:5921
  next = token.next; tokenLevel = _computePrecedence(next)
  [padrão constante: `!` pós-fixo, `!?` e `as` devolvem logo; operador com
   precedence <= tokenLevel < SELECTOR relata uma vez e zera cpc — §E.4.4]    pi:5930-5971
  para level = tokenLevel; level >= precedence; level--:
     lastBinaryExpressionLevel = -1; lastCascade = null        (reiniciados A CADA nível)
     enquanto tokenLevel == level:
        CASCADE:     !allowCascades → return token
                     `?..` e já houve cascata → NULL_AWARE_CASCADE_OUT_OF_ORDER no `?..`    pi:5982-5984
                     parseCascadeExpression
        ASSIGNMENT:  (`>>` colado a `>=` → `>>>=`)  direita: 'throw' → parseThrowExpression(allowCascades: false)
                     senão parsePrecedenceExpression(next, level, …)  (associa à direita)   pi:5988-6006
        POSTFIX:     `++`/`--` → handleUnaryPostfixAssignmentExpression; `!` → handleNonNullAssertExpression
        SELECTOR:    `.`/`?.` → parsePrimary(…, expressionContinuation) + handleEndingBinaryExpression
                                 (+ argumentos de tipo de método)
                     `(`/`[`  → parseArgumentOrIndexStar
                     `?` (já decidido `?[`) → parseArgumentOrIndexStar(checkedNullAware: true)
                     `[]` → rewriteSquareBrackets + parseArgumentOrIndexStar
                     `!` → handleNonNullAssertExpression
                     outro → UNEXPECTED_TOKEN no token; consome                             pi:6066-6070
        `is` → parseIsOperatorRest;  `as` → parseAsOperatorRest;  `?` → parseConditionalExpressionRest
        binário:     level ∈ {EQUALITY, RELATIONAL}:
                         lastBinaryExpressionLevel == level → EQUALITY_CANNOT_BE_EQUALITY_OPERAND
                                                              no operador; segue normalmente   pi:6079-6089
                         senão lastBinaryExpressionLevel = level
                     (`>>` colado a `>` → `>>>`)
                     direita = parsePrecedenceExpression(token.next, level + 1, …)  (associa à esquerda)
        next = token.next; tokenLevel = _computePrecedence(next)
     recuperação `xor/and/or/shl/shr` (abaixo)
  return token
```

Propriedades que afetam os diagnósticos:
- O `for` **desce** de nível e não sobe: depois de consumir um operador de nível baixo, um de nível mais alto
  que apareça em seguida no mesmo laço não é lido. `a = throw 1..x;` → o `throw` do lado direito é lido sem
  cascata (`pi:6002-6003`), o `..` (nível 2) aparece quando o laço externo já está no nível 1 e fica de fora:
  `expected_token @17 len 1 ';'` (no `1`) e `missing_identifier @18 len 2` (no `..`, novo comando).
- `lastBinaryExpressionLevel` vale **dentro de um nível**: `a == b != c == d` relata no 2º **e** no 3º
  operador (`@23 len 2`, `@28 len 2`); `a == b < c` não relata (níveis diferentes); o `is`/`as` não entra nessa
  conta (ramo próprio).
- `a < b > c;` e `a ? b;` em **início de comando** não chegam aqui: `computeType` os lê como declaração
  (`a<b> c`, `a? b`).
- **Operador por extenso** (`_attemptPrecedenceLevelRecovery`, `pi:6156-6226`; mapa `pi:6230-6250`): um
  identificador `xor`, `and`, `or`, `shl`, `shr` onde caberia um operador liga `_recoverAtPrecedenceLevel`
  (`pi:6290-6296`); o parser troca o token por cada candidato (`^`; `&` depois `&&`; `|` depois `||`; `<<`; `>>`)
  e reanalisa em seco; aceita se não houve erro, houve avanço e o que segue é `; , ) { } | || & &&`, EOF ou
  outra dessas palavras → `BINARY_OPERATOR_WRITTEN_OUT` na palavra, args `[palavra, operador]`. Medido:
  `var x = a and b;` → `@20 len 3 Binary operator 'and' is written as '&' instead of the written out word.`;
  `xor` → `'^'`; `shl` → `'<<'`.

##### `parseUnaryExpression` (`pi:6361-6452`)

| `next` | ação |
|---|---|
| `await` | síncrono e não `looksLikeAwaitExpression` → `parsePrimary` (identificador); senão `parseAwaitExpression` (relata `AWAIT_IN_WRONG_CONTEXT`) |
| `+` | `UnsupportedPrefixPlus` → **`MISSING_IDENTIFIER` no `+`** e identificador sintético `''` antes dele (`rewriteAndRecover`, `pi:6378-6385`): `+a` é lido como `<sintético> + a` |
| `!` `~` | operando com `parsePrecedenceExpression(op, POSTFIX_PRECEDENCE, …)` |
| `-` | idem; em padrão constante o operando recebe `numericLiteralOnly` |
| `++` `--` | idem; `handleUnaryPrefixAssignmentExpression` (nenhuma validação do alvo no parser) |
| identificador + (`.` id)? + `<…>` válido + `.` + (id \| `new`) + `(` | `parseImplicitCreationExpression` (`pi:6426-6449`) |
| outro | `parsePrimary(token, IdentifierContext.expression, cpc)` |

##### `parsePrimary` (`pi:6547-6685`)

| `next` | ação |
|---|---|
| identificador puro | `parseSendOrFunctionLiteral` |
| inteiro / hexadecimal / double / string / `#` | o literal (`parseLiteralSymbol` para `#`) |
| `true` `false` `null` | o literal |
| `this` | `parseThisExpression` (`this(` → argumentos + `handleSend`) |
| `super` | `parseSuperExpression` (`super(` → argumentos; `super?.` → `INVALID_OPERATOR_QUESTIONMARK_PERIOD_FOR_SUPER` no `?.`, `pi:6884-6886`) |
| `augment` + `super` | `parseAugmentSuperExpression` |
| `new` | `parseNewExpression` (antes, `_tryRewriteNewToIdentifier`: depois de `.` o `new` vira identificador) |
| `const` | `parseConstExpression` |
| `void` | `parseSendOrFunctionLiteral` (função nomeada com retorno `void`); senão `parseSend`: `void` está em `looksLikeStatementStart` → `MISSING_IDENTIFIER` **no `void`**, sem consumi-lo |
| `yield` / `async` fora de função síncrona | recuperação (`parseSend`) |
| `assert` | `parseAssert(token, Assert.Expression)` — lê `assert(…)` inteiro; o erro `AssertAsExpression` é descartado (§E.4.1) |
| `switch` com `allowPatterns` | `parseSwitchExpression` |
| embutido/pseudo (`isIdentifier`) | `parseSendOrFunctionLiteral` |
| **`return`** | **`UNEXPECTED_TOKEN` no `return`, pula o token e chama `parsePrimary` de novo** (`pi:6658-6662`) |
| `(` | `parseParenthesizedExpressionFunctionLiteralOrRecordLiteral` |
| `[` `[]` | `parseLiteralListSuffix` |
| `{` | `parseLiteralSetOrMapSuffix` |
| `<` | `parseLiteralListSetMapOrFunction` |
| qualquer outro (demais palavras reservadas, operadores, pontuação, EOF) | `parseSend` → `ensureIdentifier(expression)` (tabela abaixo) |

Notas do `void` (medidas): `f() { void; }` → `missing_identifier @10 len 1` (no `;`): é o caminho de
**declaração** (`computeType` dá o tipo `void` e falta o nome). `f() { var x = void; }` →
`expected_token @12 len 1 ';'` (no `=`), `missing_identifier @14 len 4` (no `void`) e, do comando `void;` que
sobra, `missing_identifier @18 len 1`. `g(void);` → `missing_identifier @9 len 4` e `expected_token @9 len 4 ')'`
(ambos no `void`; `ensureCloseParen` salta para o `)`).

##### Identificador que falta em expressão (`ExpressionIdentifierContext.ensureIdentifier`, `ic:324-377`)

| token | resultado | consome? |
|---|---|---|
| embutido/pseudo | aceito (com `await`/`yield` conferidos por `checkAsyncAwaitYieldAsIdentifier`; `await` seguido de identificador → `UNEXPECTED_TOKEN` no `await`, `ic:328-339`) | sim |
| palavra reservada dentro de `$…` de string | `EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD` | sim (`ic:348-354`) |
| palavra reservada que **não** está em `looksLikeStatementStart`, e (contexto de continuação **ou** não é `as`/`is`) | `EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD`; a palavra vira o identificador | sim (`ic:355-362`) |
| não é palavra, não é operador e não está em `. , ( ) [ ] { } ? : ;`/EOF (ex.: `=>`, `@`… ) | `MISSING_IDENTIFIER` nele; sintético inserido **depois** dele | sim (`ic:363-369`) |
| o resto: operadores, `. , ( ) [ ] { } ? : ;`, EOF, `as`/`is` fora de continuação, palavras de `looksLikeStatementStart` | `MISSING_IDENTIFIER` nele; sintético inserido **antes** | não (`ic:372-376`) |

| entrada | diagnósticos |
|---|---|
| `f() { var x = ; }` | `missing_identifier @14 len 1` (no `;`) |
| `f(x) { x = 1 + ; }` | `missing_identifier @15 len 1` |
| `f(a) { a = ; a += ; }` | `missing_identifier @11`, `@18` |
| `f(a) { a ?? ; a \|\| ; }` | `missing_identifier @12`, `@19` |
| `f() { var x = *2; }` | `missing_identifier @14 len 1` (no `*`; lê `<sintético> * 2`) |
| `f(a) { +a; }` | `missing_identifier @7 len 1` (no `+`) |
| `f() { var x = enum; }` | `expected_identifier_but_got_keyword @14 len 4` |
| `f() { var x = is int; var y = as; }` | `missing_identifier @14 len 2` (no `is`; lê `<sintético> is int`); `as` é identificador válido |
| `f() { var x = if; }` | `expected_token @12 len 1 ';'` (no `=`), `missing_identifier @14 len 2` (no `if`); depois o comando `if;`: `expected_token @16 '('`, `missing_identifier @16` |
| `f() { var x = else; }` | `expected_token @12 ';'`, `missing_identifier @14 len 4`, `unexpected_token @14 len 4 Unexpected text ';'.` |
| `f() { var x = ]; }` | `expected_token @12 ';'`, `missing_identifier @14 len 1`, `unexpected_token @14 Unexpected text ';'.` |
| `f() { var x = return; }` | `unexpected_token @14 len 6 Unexpected text 'return'.`, `missing_identifier @20 len 1` (no `;`) |
| `f() { #; }` | `missing_identifier @7 len 1` |
| `f() { var x =` (EOF) | `expected_token @12 len 1 ';'` (no `=`), `missing_identifier @13 len 0`, `expected_token @13 len 1 '}'` (scanner) |

Depois de `.`/`?.`/`..` o contexto é `expressionContinuation`, mas quem lê é `parsePrimary`, não
`ensureIdentifier`: palavra que tem ramo próprio em `parsePrimary` vira **outra expressão**, e o
`AstBuilder.doDotExpression` (`ab:824-867`) relata `MISSING_IDENTIFIER` no primeiro token dela quando o lado
direito não é `SimpleIdentifier` nem `MethodInvocation`:

| entrada | diagnósticos |
|---|---|
| `a.;` | `missing_identifier @9 len 1` (no `;`, do parser) |
| `a..;` | `missing_identifier @10 len 1` |
| `a.class;` | `expected_identifier_but_got_keyword @9 len 5` |
| `a.this;` / `a.super;` / `a.null;` | `missing_identifier` no `this` (len 4) / `super` (len 5) / `null` (len 4) — do AstBuilder |
| `a.new;` | nenhum sintático (`new` reescrito como identificador) |
| `a.1;` | `expected_token ';'` no `a` (o scanner lê `.1` como double) |
| `StringBuffer('').{ return print('0'); }.toString();` | `missing_identifier @26 len 1` (no `{`, AstBuilder: o lado direito é um literal de conjunto), `unexpected_token @28 len 6 'return'` (`parsePrimary` dentro do literal), `expected_token @45 len 1 '}'` (`pi:7087`, no `;`) |

##### `parseSend`, argumentos, índices (`pi:7620`, `pi:7783`, `pi:6454`, `pi:4234`)

- `parseSend`: recuperação `Map<…>{`/`Set<…>{`/`List<…>[`/`List[]` → `LITERAL_WITH_CLASS` no nome e lê o literal
  (`pi:7628-7659`); senão `ensureIdentifier`, argumentos de tipo só se seguidos de `(` e não recuperados, e
  `parseArgumentsOpt`.
- `parseArgumentsRest`: `)` fecha; argumento nomeado se o **segundo** token é `:` (ou, recuperação, o primeiro:
  `g(: 1)` → `missing_identifier @9` no `:`); depois da expressão: `,` continua; `)` fecha; senão, se
  `looksLikeExpressionStart(next)` → `ExpectedButGot ','` em `next` + vírgula sintética e continua
  (`pi:7814-7820`); senão `ensureCloseParen`.
- `ensureCloseParen(token, openParen)` (`pi:4234-4253`): `)` presente → ok; `openParen.endGroup` **sintético**
  (o scanner já relatou) → só move o `)` para cá (**sem erro do parser**; o erro do scanner aparece neste
  ponto, §E.4.1); senão `ExpectedButGot ')'` no token seguinte e **salta** para o `)` casado.
- Índice (`pi:6484-6497`): sem `]` → `ExpectedButGot ']'` **sempre** (mesmo com fecho sintético, ao contrário do
  `)`), depois move ou salta.
- `parseConstructorInvocationArguments`/`ensureArguments` (`pi:7219`, `pi:7209`): sem `(` → `ExpectedAfterButGot '('`
  no **último token lido** (não no seguinte) + `()` sintético; com `<…>` depois do nome do construtor →
  `CONSTRUCTOR_WITH_TYPE_ARGUMENTS`.
- Positional depois de nomeado (`g(a: 1, 2)`): nenhum erro do parser no 3.6.2 (medido).

| entrada | diagnósticos |
|---|---|
| `g(a b);` | `expected_token @17 len 1 ','` (no `b`) |
| `g(a, ; }` | `missing_identifier @15 len 1` e `expected_token @15 len 1 ')'` (este do **scanner**, no `;` para onde o `)` foi movido) |
| `g(; }` | `missing_identifier @8`, `expected_token @8 ')'` (scanner) |
| `g(a, b }` | `expected_token @18 len 1 ';'` (no `b`), `expected_token @20 len 1 ')'` (scanner, no `}`) |
| `g(a,, b); g(,);` | `missing_identifier` em cada `,` vazia (@17, @25) |
| `x = (1 }` | `expected_token @12 ';'` (no `1`), `expected_token @14 ')'` (scanner) |
| `var x = (1; }` | `expected_token @16 len 1 ')'` (scanner, no `;`) |
| `a[1; }` | `expected_token @9 ']'` duas vezes no `;` (parser + scanner) |
| `new A;` | `expected_token @10 len 1 '('` (no `A`) |
| `new;` | `missing_identifier @9` e `expected_token @9 '('` (ambos no `;`) |

##### Expressão de função e função nomeada (`pi:6687-6720`, `pi:7188-7207`, `pi:5249-5301`)

- `(`: só com `mayParseFunctionExpressions`; olha o token depois do `)` casado: `=>` ou `{` → função;
  `async`/`sync` → função; outra palavra/identificador cujo token seguinte é `=>`/`{` → função, e o
  `parseFunctionBody` relata `UNEXPECTED_TOKEN` na palavra (`pi:5467-5475`): `() asy {}` → `unexpected_token @17 len 3 'asy'`.
- `<T>(…)` (`parseLiteralFunctionSuffix`, `pi:7104-7116`): se o que segue o `)` não é corpo → `UNEXPECTED_TOKEN`
  nele e mesmo assim lê função; precedido de `const` → `UNEXPECTED_TOKEN` no `const` (`pi:7134-7137`).
- identificador ou `void` (`parseSendOrFunctionLiteral`): com `mayParseFunctionExpressions`,
  `computeType(token, false)` + nome identificador + `<…>`? + `(`…`)` + `looksLikeFunctionBody` (`{`, `=>`,
  `async`, `sync`; `pi:7961-7966`) → `parseFunctionLiteral` → **`NAMED_FUNCTION_EXPRESSION`** (ficha em §E.4.6).
- `mayParseFunctionExpressions` é `false` na lista de inicializadores de construtor (`pi:3996`, restaurado
  `pi:4036`) e nos padrões/guardas dos casos de `switch` expressão (`pi:10367`, `pi:10403`); volta a `true`
  dentro de `(…)`, `[…]`, `{…}`, argumentos, índice, `assert(…)` e interpolação (`pi:6715`, `pi:6933`,
  `pi:7001`, `pi:7789`, `pi:6480`, `pi:9147`, `pi:7504`). `A() : f = g() {}` → `g()` é chamada e `{}` o corpo
  (nenhum erro sintático, medido).
- Corpo (`parseFunctionBody`, `pi:5415-5504`): `;` em expressão de função → `MISSING_FUNCTION_BODY` no `;`;
  `=` → idem + `=>` sintético; **`return` no lugar de `=>`/`{`** → `MISSING_FUNCTION_BODY` no `return` + `=>`
  sintético e a expressão começa **depois** do `return` (`pi:5458-5463`; só alcançado quando a função já foi
  decidida — declaração `f() return 1;`, `<T>() return …` —: `g(() return 1)` não é lido como função e dá
  `expected_token @12 len 6 ')'` no `return`); outro token →
  `ExpectedFunctionBody` (`MISSING_FUNCTION_BODY`) nele + `{}` sintético.

##### `this`/`super` em expressão e em inicializador (`pi:6860-6888`, `pi:4052-4194`, `ab:638-777`)

`parseSuperExpression` só relata `super?.`. O resto é do AstBuilder: `super` como operando →
`MISSING_ASSIGNABLE_SELECTOR` (`reportErrorIfSuper`); em inicializador → `INVALID_SUPER_IN_INITIALIZER`
/ `INVALID_THIS_IN_INITIALIZER` (ficha em §E.4.6, com o pseudocódigo de `parseInitializer`). Medidos:
`f() { super; super = 1; super(); }` → `missing_assignable_selector @6 len 5`;
`illegal_assignment_to_non_assignable @13 len 5` + `missing_assignable_selector @13 len 5`; o `super()` não
tem erro sintático. `class A { m() { super?.m(); } }` → `invalid_operator_questionmark_period_for_super @21 len 2`.
`class A { var x; A() : this.x = super; }` → `missing_assignable_selector @32 len 5` (não é
`invalid_super_in_initializer`: o `super` é o lado **direito** de um inicializador de campo válido).

##### Cascata, `new`/`const`, literais, condicional, `is`/`as`

- **Cascata** (`pi:6302-6359`): depois de `..`/`?..`: `[` → índice; senão `parseSend(expressionContinuation)`
  (logo `a..;` é `MISSING_IDENTIFIER` no `;`). Laço de `.id`/`?.id`, `!`, argumentos de tipo, chamadas e
  índices; no fim, atribuição com `parseExpressionWithoutCascade`. `a..b?..c` →
  `null_aware_cascade_out_of_order @11 len 3`. Sem cascata (`allowCascades: false`): ramos do condicional,
  lado direito de atribuição dentro de cascata, `throw` à direita de atribuição, padrões constantes.
- **`new`** (`pi:7246-7317`): `new Map<…>{`, `new Set<…>{`, `new List<…>[`/`[]` (nome não seguido de `.`) →
  `LITERAL_WITH_CLASS_AND_NEW` de `new` até o nome (`reportRecoverableErrorWithEnd`) e lê o literal;
  `new {`, `new [`, `new <…>{`/`[` → `LITERAL_WITH_NEW` no `new`. Medido:
  `new Map<int, int>{}; new List[]; new <int>[];` → `literal_with_class_and_new @6 len 7`, `@27 len 8`,
  `literal_with_new @39 len 3`.
- **`const`** (`pi:7347-7453`): `[`/`[]` lista; `(` record (`const (1)` → `RECORD_LITERAL_ONE_POSITIONAL_NO_TRAILING_COMMA`
  no `)`); `{` conjunto/mapa; `<` → `parseLiteralListSetMapOrFunction`; `const Map<…>{`/`const List<…>[` →
  `LITERAL_WITH_CLASS`; senão construtor com argumentos obrigatórios.
- **Lista** (`pi:6917-6986`) e **conjunto/mapa** (`pi:6990-7098`): depois de um elemento, sem `,` nem fecho:
  `looksLikeLiteralEntry(next)` (`fe:parser/literal_entry_info.dart:75-81`: `looksLikeExpressionStart`, `...`,
  `...?`, `if`, `for`, `await for`) → `ExpectedButGot ','` em `next` (ou `EXPECTED_ELSE_OR_COMMA` se o
  elemento era um `if` sem `else`, `ifCount > 0`) + vírgula sintética; senão, lista com `]` sintético → só
  move (erro só do scanner); lista com `]` real → `ExpectedButGot ']'` e salta; mapa → `ExpectedButGot '}'`
  em `next` e salta para o `}` casado. `<…>` sem `[`/`{` depois → `ExpectedButGot '['` (`pi:7151-7156`);
  mais de 2 argumentos de tipo antes de `{` → mensagem descartada (§E.4.1).

  | entrada | diagnósticos |
  |---|---|
  | `[1 2]` | `expected_token @17 len 1 ','` (no `2`) |
  | `[if (true) 1 2]` | `expected_else_or_comma @27 len 1` |
  | `{1, 2 3}` | `expected_token @20 len 1 ','` |
  | `f() { [1, 2 }` | `expected_token @10 ';'` (no `2`), `expected_token @12 ']'` (scanner, no `}`) |
  | `f() { [1, 2` (EOF) | `expected_token @10 ';'`, `expected_token @11 ']'` e `'}'` (scanner; sem quebra de linha final) |
  | `[,]; [1,,2];` | `missing_identifier @7`, `@14` (nas vírgulas) |
  | `var l = <int>;` | `expected_token @19 len 1 '['` (no `;`) |
  | `{1: 2, 3 }` | nenhum sintático (`ambiguous_set_or_map_literal_both` da resolução) |
  | `<int, int, int>{}` | nenhum sintático; `expected_two_map_type_arguments @14 len 15` (verificador) |

- **Condicional** (`pi:5877-5887`): `?` → `parseExpressionWithoutCascade`, `ensureColon`,
  `parseExpressionWithoutCascade`. `var x = a ? b;` → `expected_token @23 len 1 ':'` e
  `missing_identifier @23 len 1` (ambos no `;`); `g(a ? b)` → os dois no `)`; `a ? b : ;` →
  `missing_identifier` no `;`; `a ? : b` → `missing_identifier` no `:`.
- **`is`/`as`** (`pi:7839-7932`): `computeTypeAfterIsOrAs` (o `?` faz parte do tipo só diante de
  `) } ] ? ?? , ; : is as .. || &&` ou EOF; diante de `{`/`when` decide por `canParseAsConditional`);
  `ensureTypeNotVoid`. Depois, `skipChainedAsIsOperators`: cada `is`/`as` seguinte é `UNEXPECTED_TOKEN`
  **no operador**, o `!` e o tipo são **pulados** (não entram na árvore): `a is B is C as D;` →
  `unexpected_token @14 len 2 'is'`, `@19 len 2 'as'`; `a is! B is! C` → `@15 len 2 'is'`;
  `a as B as C` → `@14 len 2 'as'`.
- **`>>` em genéricos**: o scanner entrega `>>`/`>>>` inteiros; `computeTypeParamOrArg(…).parseArguments`
  parte o token ao ler argumentos de tipo (`<List<List<int>>>[]` não tem erro; a metadata `@A<List<int>>`
  relata no **segundo** `>` partido, §E.4.6). Em expressão, `>>` colado a `>`/`>=` só é juntado quando o
  scanner não produz `>>>` (recurso triple-shift desligado; `pi:5992-6001`, `pi:6090-6101`); `a >> > b` (com espaço) →
  `missing_identifier @24 len 1` no `>` solto.


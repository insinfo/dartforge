#### E.4.6 Códigos

Nos sete códigos marcados "sem emissor", a constante existe em `crates/diagnostics/src/codigos_g.rs` e nenhum
arquivo de `crates/` a usa (busca feita nesta sessão).

##### `missing_statement` (perda 5: FN 1, FP 4, msg 0, pos 0)
- **Emissão:** `AstBuilder.handleExpressionStatement` (`ab:4238-4248`), fase AstBuilder, chamado por
  `parseExpressionStatement` (`pi:5789-5798`) depois do `ensureSemicolon`. O parser fasta não emite
  (corrige §E: "`parseStatement` → `ExpectedStatement`").
- **Condição exata:** `expression is SimpleIdentifier && expression.token.keyword?.isBuiltInOrPseudo == false`.
  O token precisa ter `keyword` (identificador comum e identificador sintético têm `keyword == null` →
  `null == false` é falso) e ser palavra **reservada**. Só chega a `SimpleIdentifier` a palavra reservada que
  `ExpressionIdentifierContext` aceita como identificador (fora de `looksLikeStatementStart`, não `as`/`is`) e
  que não tem ramo em `parseStatementX`/`parsePrimary`: `case`, `catch`, `class`, `default`, `enum`, `extends`,
  `finally`, `in`, `with` (medidos: `case`, `class`, `default`, `extends`, `finally`, `in`, `with`), e só se
  **nada** a estende (sem `(`, `.`, `=`, operador): `catch (e)` é chamada, `class = 10` é atribuição.
- **Posição:** o token da palavra (`beginToken` = `endToken`), len do lexema.
- **Mensagem:** `Expected a statement.` (front_end, índice 29); sem argumentos.
- **Supressões e ordem:** sempre acompanhado, no mesmo token, de `EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD` (parser,
  antes) e de `UNDEFINED_IDENTIFIER` (resolução); `EXPECTED_TOKEN ';'` no mesmo token se o seguinte não é `;`.
  Não sai para `)`, `]`, `else`, `}`, EOF (identificador sintético), nem para `void;`, `int;`, `this;`.
- **No DartForge:** `erro_statement` (`crates/frontend/src/parser/mod.rs:583-590`), chamado em
  `statements.rs:277-279` (`)`, `]`, `}`, EOF, script) e `statements.rs:354-364` (`case catch class default else
  enum extends finally in is with`), e `statements.rs:447` (anotação sem declaração). Diferenças: (1) relata
  `MISSING_STATEMENT` para `)`/`]`/`else`/`is`, que no fasta dão `MISSING_IDENTIFIER` + `UNEXPECTED_TOKEN`
  (ou, `is`, `MISSING_IDENTIFIER` + `EXPECTED_TYPE_NAME`); (2) relata mesmo quando a palavra é seguida de algo
  (`class = 10`, `catch (e)`); (3) devolve `Err` e `synchronize_statement` (`statements.rs:196-220`) pula até o
  `;`, engolindo o que o fasta leria. Amostras: FP `class/keyword_test.dart:10:7` (`int class = 10;`: o fasta dá
  `expected_token` no `int` e `expected_identifier_but_got_keyword` no `class`, sem este código); FP ×3
  `UnusedLocalVariable__forPartsWithPatter*.dart:2:25` (`for (var (_,) = (0,);;) {}`: `statements.rs:85-87`
  recusa o `=` depois do padrão, a sincronização consome até o 1º `;`, o 2º vira comando vazio e o `)` cai em
  `statements.rs:277`; no fasta o código é válido); FN `label/label8_test.dart:14:5` (`switch(i) L: { case 111:`:
  `parse_switch` exige `{` em `statements.rs:716` e aborta; no fasta o bloco rotulado é um bloco comum e o
  `case` dentro dele é comando de expressão). Mudança: comando de expressão como recuperação universal
  (identificador sintético + `;` sintético, sem `Err`), `MISSING_STATEMENT` só pela condição do AstBuilder, e a
  regra "sem progresso" de §E.4.2.
- **Exemplos (oráculo vivo 3.6.2):**
```dart
f() { case 1: }
```
  `expected_identifier_but_got_keyword @6 len 4 (1:7)`, `expected_token @6 len 4 (1:7) Expected to find ';'.`,
  `missing_statement @6 len 4 (1:7) Expected a statement.`, `undefined_identifier @6 len 4`,
  `expected_token @11 len 1 (1:12) ';'` (no `1`), `missing_identifier @12 len 1 (1:13)`,
  `unexpected_token @12 len 1 (1:13) Unexpected text ';'.`
```dart
f(x) { switch (x) L: { case 1: break; } }
```
  `expected_body @16 len 1 (1:17) A switch statement must have a body, even if it is empty.`,
  `expected_identifier_but_got_keyword @23 len 4`, `expected_token @23 len 4 ';'`, `missing_statement @23 len 4 (1:24)`,
  `expected_token @28 len 1 ';'`, `missing_identifier @29 len 1`, `unexpected_token @29 len 1 Unexpected text ':'.`,
  `break_outside_of_loop @31 len 5` (o `switch` já acabou no bloco sintético).
  Outros: `f() { default }` → `…got_keyword`, `expected_token ';'`, `missing_statement`, todos `@6 len 7`;
  `f() { in; }` → `missing_statement @6 len 2`; `f() { this; null; true; 1; 'a'; class; }` → só no `class`
  (`@32 len 5`); `f() { extends; is; new; with; }` → `missing_statement @6 len 7` e `@24 len 4` (o `is` dá
  `missing_identifier @15 len 2` + `expected_type_name @17 len 1`; o `new`, `missing_identifier @22` +
  `expected_token @22 '('`); `f() { int class = 10; }` → `expected_token @6 len 3 ';'`,
  `expected_identifier_but_got_keyword @10 len 5` (sem `missing_statement`).

##### `unexpected_token` (perda 7: FN 7, FP 0, msg 0, pos 0)
- **Emissão:** parser fasta, `templateUnexpectedToken`; via `reportByCode` (`ec:470`). Pontos de emissão em
  comandos e expressões: `parsePrimary` com `return` (`pi:6658-6662`); `skipChainedAsIsOperators` (`pi:7923`);
  os três laços "sem progresso" (`pi:5493`, `pi:8635`, `pi:9093`); sobra em `assert(…)` (`pi:9171`); seletor
  desconhecido (`pi:6068`); sobra no cabeçalho do `for` (`pi:8501`) e entre o nome e o `in` (`pi:8558`);
  `set` + identificador em comando (`pi:5680`); `case` em `switch` expressão (`pi:10378`); token depois de
  `<T>(…)` que não é corpo (`pi:7113`) e `const` antes de `<T>(` (`pi:7135`); palavra solta antes de `=>`/`{`
  no corpo de função (`pi:5468`, `pi:5472`); `await` seguido de identificador (`ic:333`);
  `skipUnexpectedTokenOpt` (`pi:4344`, cabeçalhos — fora desta parte).
- **Condição exata:** a de cada ponto (§E.4.2–E.4.4).
- **Posição:** o token relatado (len do lexema); com token sintético, o primeiro token real seguinte.
- **Mensagem:** `Unexpected text '{0}'.` (`analyzer/messages.yaml:21585`); `{0}` = lexema do token do
  **template** — `';'` no laço do corpo de função (§E.4.2).
- **Supressões e ordem:** nos laços "sem progresso" vem depois de `MISSING_IDENTIFIER` e `EXPECTED_TOKEN ';'`;
  `return` em expressão não gera mais nada se o resto é expressão válida.
- **No DartForge:** emissores: `expressions.rs:685-694` (`is`/`as` encadeados — igual ao fasta),
  `statements.rs:867-875` (`assert`), `expressions.rs:1672-1675` (`case` em `switch` expressão),
  `declarations.rs:173`, `:1415`, `:1435`, `:3185`. Amostras (todas FN): `UnawaitedReturnInTryBlock__insideClosur_c0f7d494.dart:3:25`
  (`() async => return Future.value(null)`): `parse_primary` cai em `_ => erro_identificador`
  (`expressions.rs:1305`, `mod.rs:594-600`) → `EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD` e `Err` (é o FP desse código
  em §E); falta o ramo `return`. `anonymous_methods/block/error_test.dart:48:22` e `:54:22`
  (`StringBuffer('').{ return print('0'); }.toString();`, oráculo 3.6.2): o lado direito do `.` é lido por
  `parsePrimary` como literal de conjunto; nós exigimos nome de membro (`member_name`, `expressions.rs:1077`).
  `label/label8_test.dart:14:13` (`:` depois de `case 111` dentro de bloco): regra "sem progresso" do bloco.
  `patterns/invalid_const_pattern_binary_test.dart:399:16` (`case 1 ?? 2:`) e
  `patterns/invalid_const_pattern_test.dart:145:25` (`case void fun() {}:`): o `:` original sobra como comando
  depois do `:` sintético de `ensureColon`; nós paramos em `expect_op(Op::Colon)` (`statements.rs:761`) com
  `Err`. `primary_constructors/syntax/empty_body_error_test.dart:18:6` (`enum const E3;`, "Unexpected text
  'const'"): oráculo 3.13.4; no 3.6.2 vivo a mesma entrada dá `missing_identifier @5 len 5` e
  `missing_enum_body @5 len 5`, sem este código — o ponto de emissão no parser 3.13 não foi verificado.
- **Exemplos (oráculo vivo 3.6.2):**
```dart
f() => return 1;
```
  `unexpected_token @7 len 6 (1:8) Unexpected text 'return'.` (único diagnóstico)
```dart
void foo() {
  try {
    var x = () async => return Future.value(null);
  } catch (_) {}
}
```
  `unexpected_token @45 len 6 (3:25) Unexpected text 'return'.` (+ `unused_local_variable @29`)
```dart
f(a) { a is B is C as D; }
```
  `unexpected_token @14 len 2 (1:15) Unexpected text 'is'.`, `unexpected_token @19 len 2 (1:20) Unexpected text 'as'.`
  Outros: `g(() => return 1);` → `@15 len 6`; `f() { set x = 1; }` → `@6 len 3 'set'`; os de §E.4.2–E.4.4.

##### `break_outside_of_loop` (perda 1: FN 1, FP 0, msg 0, pos 0)
- **Emissão:** `parseBreakStatement` (`pi:9111-9124`), parser fasta; tabela de índices (52).
- **Condição exata:** `!token.next.isIdentifier && loopState == LoopState.OutsideLoop` (`isBreakAllowed`,
  `pi:377`). Estado conforme a tabela de §E.4.2: zera em todo corpo de função em bloco; `switch` comando e
  laços o ligam; `if`, `try`, bloco, rótulo e `switch` expressão não.
- **Posição:** a palavra `break` (len 5).
- **Mensagem:** `A break statement can't be used outside of a loop or switch statement.` Sem argumentos.
- **Supressões e ordem:** com rótulo (`break L;`) o parser nunca relata; a resolução dá `LABEL_UNDEFINED` se
  for o caso. O comando é construído e o fluxo o trata normalmente (`dead_code` depois dele).
- **No DartForge:** sem emissor; `statements.rs:298-307` lê `break` sem estado. Amostra FN
  `loop/break_outside_loop_test.dart:8:5` (`if (true) { break; }`). Mudança: campo `loop_state` no `Parser`
  com os mesmos três valores, salvo/restaurado em `parse_for_rest` (`statements.rs:631`), `parse_while` (`:682`),
  `parse_do` (`:692`), `parse_switch` (`:709`) e no corpo em bloco de `parse_function_body_ex` (`declarations.rs:3110`).
- **Exemplos (oráculo vivo 3.6.2):**
```dart
f() { while (true) { () { break; }; } }
```
  `break_outside_of_loop @26 len 5 (1:27) A break statement can't be used outside of a loop or switch statement.`
  Outros: `f() { break; }` → `@6 len 5`; `f() { if (true) break; else continue; }` → `break_outside_of_loop @16 len 5`,
  `continue_outside_of_loop @28 len 8`; `var v = () { break; };` → `@13`; `f() { while (true) { void g() { break; } } }`
  → `@32`; `f() { do { break; } while (true); break; }` → só o 2º (`@34`); `f(x) { switch (x) { case 1: break; } break; }`
  → só o 2º (`@37`); `f() { L: { break L; break; } }` → só o sem rótulo (`@20`); `f(x) { if (x case 1) { break; } }` → `@23`;
  `f() { try { continue; } finally { break; } }` → `continue_outside_of_loop @12 len 8`, `break_outside_of_loop @34 len 5`;
  `class A { m() { continue; break; } }` → `@16 len 8`, `@26 len 5`.

##### `continue_outside_of_loop` (perda 1: FN 1, FP 0, msg 0, pos 0)
- **Emissão:** `parseContinueStatement` (`pi:9204-9225`); índice 2.
- **Condição exata:** com rótulo: `loopState == OutsideLoop` (`isContinueWithLabelAllowed`, `pi:381`,
  `pi:9211-9213`); sem rótulo: `loopState == OutsideLoop` (com `InsideSwitch` sai o código seguinte).
- **Posição:** a palavra `continue` (len 8), mesmo quando há rótulo.
- **Mensagem:** `A continue statement can't be used outside of a loop or switch statement.`
- **Supressões e ordem:** a resolução acrescenta `LABEL_UNDEFINED` (rótulo fora de escopo) ou
  `CONTINUE_LABEL_INVALID` (rótulo de bloco), no rótulo / no comando.
- **No DartForge:** sem emissor (`statements.rs:308-317`). Amostra FN `label/label3_test.dart:10:3`
  (`continue L;` depois do `while` rotulado). Mesma mudança do código anterior.
- **Exemplos (oráculo vivo 3.6.2):**
```dart
main() { L: while (false) { if (true) break L; } continue L; }
```
  `continue_outside_of_loop @49 len 8 (1:50) A continue statement can't be used outside of a loop or switch statement.`,
  `label_undefined @58 len 1 (1:59) Can't reference an undefined label 'L'.`
  Outros: `f() { continue; }` → `@6 len 8`; `f() { L: { continue L; } }` → `continue_outside_of_loop @11 len 8` +
  `continue_label_invalid @11 len 11`; `f(x) { switch (x) { case 1: () { continue; }; } }` → `@33 len 8` (a função
  zera o estado: não é `…_in_case`); `f(x) { var y = switch (x) { _ => () { continue; } }; }` → `@38 len 8`;
  `f() { break L; continue L; }` → só `continue_outside_of_loop @15 len 8` do parser (+ `label_undefined` ×2).

##### `continue_without_label_in_case` (perda 1: FN 1, FP 0, msg 0, pos 0)
- **Emissão:** `parseContinueStatement` (`pi:9215-9220`); índice 64.
- **Condição exata:** `continue` **sem rótulo** e `loopState == LoopState.InsideSwitch` — isto é, dentro de um
  `switch` comando que não está dentro de laço, sem função em bloco no meio.
- **Posição:** a palavra `continue` (len 8).
- **Mensagem:** `A continue statement in a switch statement must have a label as a target.`
- **Supressões e ordem:** `switch` dentro de laço mantém `InsideLoop` (sem erro: o `continue` é do laço); laço
  dentro do `case` volta a permitir; `continue L;` nunca dá este código (rótulo de `case` é válido; rótulo
  inválido é da resolução). Independe da versão da linguagem.
- **No DartForge:** sem emissor. Amostra FN `switch/switch3_test.dart:12:20`. Mesma mudança.
- **Exemplos (oráculo vivo 3.6.2):**
```dart
f(x) { switch (x) { case 1: continue; } }
```
  `continue_without_label_in_case @28 len 8 (1:29) A continue statement in a switch statement must have a label as a target.`
  Sem diagnóstico: `f(x) { switch (x) { case 1: while (true) { continue; } } }`;
  `f(x) { while (true) { switch (x) { case 1: continue; } } }`;
  `f(x) { switch (x) { case 1: for (;;) { switch (x) { case 2: continue; } } } }`;
  `f(x) { switch (x) { case 1: continue L; L: case 2: break; } }`. `f() { L: switch (1) { case 1: continue L; } }`
  → só `continue_label_invalid @30 len 11` (resolução).

##### `named_function_expression` (perda 1: FN 1, FP 0, msg 0, pos 0)
- **Emissão:** `parseNamedFunctionRest` com `isFunctionExpression` (`pi:5286-5289`), chamado por
  `parseFunctionLiteral` (`pi:5249`) ← `parseSendOrFunctionLiteral` (`pi:7188-7207`) ← `parsePrimary`
  (identificador, embutido ou `void`); via `reportByCode` (`ec:372`).
- **Condição exata:** `mayParseFunctionExpressions` e, em posição de primária:
  `typeInfo = computeType(token, required: false)`; o token depois do tipo é identificador; depois dele
  `<…>` opcional e `(`; o token depois do `)` casado é `{`, `=>`, `async` ou `sync`.
- **Posição:** o **nome** (`beforeName.next`), len do nome.
- **Mensagem:** `Function expressions can't be named.` (`analyzer/messages.yaml:21520`); correção `Try removing
  the name, or moving the function expression to a function declaration statement.`
- **Supressões e ordem:** não sai onde `mayParseFunctionExpressions` é falso (inicializadores de construtor:
  `A() : f = g() {}`; padrão/guarda de `switch` expressão). Em comando, `n() {}` é função local (sem erro). O
  AstBuilder monta a expressão de função sem o nome. Único diagnóstico sintático em todos os casos medidos.
- **No DartForge:** sem emissor. `parse_primary` lê `Kind::Ident` como `Identifier` (`expressions.rs:1181-1184`)
  e os seletores leem `(…)` como chamada; o `{` que segue quebra o comando. `void` em primária cai em
  `_ => erro_identificador` (`expressions.rs:1305`). Amostra FN `patterns/invalid_const_pattern_test.dart:305:23`
  (`case const void fun() {}:`). Mudança: em `parse_primary` (identificador e `void`), o lookahead do fasta
  (tipo opcional, nome, `<…>`?, `(…)`, corpo) → ler como expressão de função e relatar no nome; respeitar um
  `may_parse_function_expressions` desligado em `parse_initializer` (`declarations.rs:2924`).
- **Exemplos (oráculo vivo 3.6.2):**
```dart
f() { var f = void g() {}; }
```
  `named_function_expression @19 len 1 (1:20) Function expressions can't be named.` (+ `unused_local_variable @10`)
```dart
f(o) { switch (o) { case const void fun() {}: } }
```
  `named_function_expression @36 len 3 (1:37)`, `constant_pattern_with_non_constant_expression @39 len 5`
  Outros: `var f = g() {};` → `@14 len 1`; `var f = int g<T>() => 1;` → `@18 len 1`; `g(void h() {});` → `@14`;
  `g(h() async {}); g(i() => 1); g(j());` → `@9`, `@26` (e `j()` é chamada); `[void k() {}]`, `{'a': m() {}}` →
  `@20`, `@43`; em 2.19, `case void fun() {}:` → `@64 len 3`.

##### `invalid_super_in_initializer` (perda 1: FN 1, FP 0, msg 0, pos 0)
- **Emissão:** **AstBuilder**, `buildInitializerTargetExpressionRecovery` (`ab:734-777`, relato `ab:751-755`), chamado
  por `buildInitializer` (`ab:638-732`) em `endInitializers` (`ab:2287-2314`); índice 47. O parser não emite.
- **Condição exata:** o parser lê cada inicializador como **expressão** (`parseInitializer`, `pi:4052-4139`):
```
'assert' → parseAssert(Initializer)
'super'  → parseSuperInitializerExpression (pi:4145-4188):
             [`.` nome  (não identificador → ensureIdentifier(expressionContinuation))]
             depois: '(' → ok;  `?.` → pula `?.` e o nome (o erro sai em parseSuperExpression)
                     '=' → depois de `super.x`: FIELD_INITIALIZED_OUTSIDE_DECLARING_CLASS em x
                     outro → ExpectedAfterButGot '(' no token seguinte + `()` sintético
             → parseExpression desde o `super`
'this'   → `this.x =` → expressão;  `this(` / `this.n(` → expressão (+ REDIRECTING_CONSTRUCTOR_WITH_BODY se
             segue `{`/`=>`);  `this` sem `.`/`(` → ExpectedButGot '.' no token seguinte;
             tudo o que não fechou acima → MISSING_ASSIGNMENT_IN_INITIALIZER no `this` com `<sintético> =` antes
identificador → `x =` → expressão; senão MISSING_ASSIGNMENT_IN_INITIALIZER no nome
outro    → MISSING_INITIALIZER no token anterior + `<sintético> = <sintético>`
```
  e `buildInitializer` classifica o nó: `super(…)`/`this(…)` (`FunctionExpressionInvocation` cuja função é o
  próprio `super`/`this`), `super.n(…)`/`this.n(…)` (`MethodInvocation` com alvo direto), atribuição
  (`x = e`, `this.x = e`) e `assert` são válidos. `MethodInvocation` com outro alvo, `PropertyAccess`,
  `IndexExpression` e `CascadeExpression` vão para a recuperação, que desce pela cadeia
  (`FunctionExpressionInvocation.function`, `MethodInvocation.target`, `PropertyAccess.target`): se a base é
  `SuperExpression` → **este código** e o nó vira `SuperConstructorInvocation` com a última lista de
  argumentos vista; se é `ThisExpression` → `INVALID_THIS_IN_INITIALIZER`; senão (e qualquer outro nó) →
  `INVALID_INITIALIZER` no nó inteiro.
- **Posição:** a palavra `super` (len 5).
- **Mensagem:** `Can only use 'super' in an initializer for calling the superclass constructor (e.g. 'super()'
  or 'super.namedConstructor()')` (sem ponto final).
- **Supressões e ordem:** sai depois dos erros do parser do mesmo inicializador. Não sai para `super;` /
  `super.foo;` (o `()` sintético torna o nó válido: só `EXPECTED_TOKEN '('`), `super.x = 1`, `super?.foo()`
  (alvo direto), nem para `super` no lado direito (`this.x = super` → `MISSING_ASSIGNABLE_SELECTOR`).
- **No DartForge:** sem emissor. `parse_initializer` (`declarations.rs:2924-3008`) lê `super` + (`.` nome)? +
  `parse_arguments` de forma estrutural: nome com palavra reservada (`identifier_or_new`, `:2934`) e qualquer
  sufixo depois dos argumentos são `Err`. Amostra FN `invalid_super_in_initializer/InvalidSuperInInitializer__constructor__36d245d1.dart:2:9`
  (`C() : super.const();`): no fasta, `const` é aceito como nome com `EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD`
  (`ic:359`), mas a expressão é relida por `parsePrimary`, que vê `const (…)` = record constante vazio; o
  `doDotExpression` dá `MISSING_IDENTIFIER` no `const` e o nó é `PropertyAccess(super, const)` → este código.
  Mudança: ler o inicializador `super…`/`this…` como expressão (os seletores já existem em `parse_selectors`,
  `expressions.rs:955`) e classificar pela forma da árvore, como `buildInitializer`.
- **Exemplos (oráculo vivo 3.6.2):**
```dart
class A { A() : super.const(); }
```
  `invalid_super_in_initializer @16 len 5 (1:17) Can only use 'super' in an initializer for calling the superclass constructor (e.g. 'super()' or 'super.namedConstructor()')`,
  `expected_identifier_but_got_keyword @22 len 5 (1:23) 'const' can't be used as an identifier because it's a keyword.`,
  `missing_identifier @22 len 5 (1:23) Expected an identifier.`
  Outros: `A() : super()..foo;` → só `@16 len 5`; `A() : super[0];` → `@16 len 5` + `expected_token @21 len 1 '('`;
  `A() : super.a.b();` → `@16 len 5` + `expected_token @23 len 1 '('` (no 2º `.`); `A() : x = 1, super.a().b;` →
  `@32 len 5`; `A() : super().b, super()();` → `@16 len 5` e `invalid_initializer @27 len 9`;
  `A() : super;` → só `expected_token @21 len 1 '('`; `A() : super.foo;` → só `expected_token @25 '('`;
  `A() : super.x = 1;` → `field_initialized_outside_declaring_class @22 len 1`;
  `A() : super?.foo();` → `invalid_operator_questionmark_period_for_super @21 len 2`;
  `A() : this().b;` e `A() : this.n()..b;` → `invalid_this_in_initializer @16 len 4`;
  `A() : this.a.b();` → `missing_assignment_in_initializer @16 len 4` (não é `invalid_this…`);
  `A() : this;` → `missing_assignment_in_initializer @23 len 4` + `expected_token @27 len 1 '.'`;
  `A() : ;` → `missing_initializer @14 len 1` (no `:`); `A() : x, y = 1;` → `missing_assignment_in_initializer @26 len 1`.

##### `annotation_with_type_arguments_uninstantiated` (perda 2: FN 2, FP 0, msg 0, pos 0)
- **Emissão:** **parser fasta**, `parseMetadata` (`pi:1346-1349`); índice 114. Corrige §E ("Cauda"), que
  apontava o AstBuilder e o `<…>`.
- **Condição exata:** `hasTypeArguments` (`<` logo depois do nome ou do nome qualificado) e, depois dos
  argumentos de tipo e do `.nome` opcional, o token seguinte **não** é `(` (colado ou não).
- **Posição:** `token` = o **último token lido da anotação**: o `>` que fecha os argumentos de tipo (com `>>`,
  o segundo `>` partido, len 1) ou, se houve `.nome`, esse nome (`new` inclusive).
- **Mensagem:** `An annotation with type arguments must be followed by an argument list.`
- **Supressões e ordem:** com `(` separado por espaço sai `ANNOTATION_SPACE_BEFORE_PARENTHESIS` no lugar. A
  resolução acrescenta `NO_ANNOTATION_CONSTRUCTOR_ARGUMENTS` na anotação inteira (não sai quando o nome é um
  getter estático: `@A.b<int>`). Em < 2.14, mais `EXPERIMENT_NOT_ENABLED` no `<`.
- **No DartForge:** sem emissor. `parse_metadata` (`declarations.rs:698-732`) já lê `<…>` e o `.nome`/`.new`
  (`:708-717`) e só lê argumentos se o `(` está colado ou há argumentos de tipo (`:718-723`); falta relatar
  quando `type_args` não é vazio e o token corrente não é `(`, no token anterior (`tokens[pos - 1]`), e
  `ANNOTATION_SPACE_BEFORE_PARENTHESIS` (também ausente) no `(` separado quando há argumentos de tipo ou o que
  segue o `)` é `class`/`enum`. Amostras FN `metadata/constructor_new_error_test.dart:25:28` e `:43:33`
  (`@GenericClass<int, String>.new`, `@self.GenericClass<int, String>.new`: no `new`).
- **Exemplos (oráculo vivo 3.6.2):**
```dart
class G<T, S> { const G(); }
@G<int, String>.new
void f() {}
```
  `no_annotation_constructor_arguments @29 len 19 (2:1)`,
  `annotation_with_type_arguments_uninstantiated @45 len 3 (2:17) An annotation with type arguments must be followed by an argument list.`
  Outros (classe `A<T>` com construtor `const`): `@A<int> class C {}` → `@32 len 1` (no `>`);
  `@A<int>.n class C {}` → `@47 len 1` (no `n`); `@A<List<int>> class C {}` → `@38 len 1` (2º `>` de `>>`);
  `@A.b<int> class C {}` → `@54 len 1`, sem o erro de resolução; local `@A<int> var x = 1;` → `@38 len 1`;
  em `// @dart=2.12`, `@A<int> class D {}` → `experiment_not_enabled @63 len 1` + `…_uninstantiated @67 len 1`.

##### `empty_record_type_named_fields_list` (perda 2: FN 2, FP 0, msg 0, pos 0)
- **Emissão:** parser fasta, `parseRecordTypeNamedFields` (`pi:1743-1745`); índice 129.
- **Condição exata:** dentro de um tipo record, `{` seguido direto de `}` (`parameterCount == 0`).
- **Posição:** o **`}`** (len 1) — `token = next` em `pi:1741` antes do relato. Corrige §E ("no `{`").
- **Mensagem:** `The list of named fields in a record type can't be empty.`
- **Supressões e ordem:** único erro do tipo: `({})` conta um "parâmetro" com `hasNamedFields`, logo não sai
  `RECORD_TYPE_ONE_POSITIONAL_NO_TRAILING_COMMA`. Vale em qualquer posição de tipo (variável, parâmetro,
  `typedef`).
- **No DartForge:** sem emissor. `parse_record_type` (`crates/frontend/src/parser/types.rs:207-253`) aceita
  `{}` em silêncio (`:216-219`). Amostras FN `record_type_empty_problems_test.dart:6:26` e
  `record_type_problems_test.dart:6:26` (`(int, int, {/*missing*/}) r1`). Mudança: relatar no `}` quando o
  grupo nomeado fecha sem campo.
- **Exemplos (oráculo vivo 3.6.2):**
```dart
main() {
  (int, int, {/*missing*/}) r1 = (1, 2);
}
```
  `empty_record_type_named_fields_list @34 len 1 (2:26) The list of named fields in a record type can't be empty.`
  Outros: `({}) r = ();` → `@2 len 1`; `(int, {}) r = (1,);` → `@7 len 1`; `f() { ({}) r; (int x, {}) s; }` →
  `@17`, `@32`; `typedef S = ({});` → `@33` (linha 2 do caso); parâmetro `({}) q` → `@50`.

##### `record_type_one_positional_no_trailing_comma` (perda 2: FN 2, FP 0, msg 0, pos 0)
- **Emissão:** parser fasta, `parseRecordType` (`pi:1677-1680`); índice 131.
- **Condição exata:** `parameterCount == 1 && !hasNamedFields && !sawComma`: exatamente um campo posicional
  (com ou sem nome) e nenhuma vírgula. Só depois de `computeType` ter decidido que o `(` é tipo (por exemplo
  porque depois do `)` vem um nome): `(int) x;`, `(int x) y`, `typedef R = (int);`, parâmetro `(int) p`.
- **Posição:** o `)` (len 1).
- **Mensagem:** `A record type with exactly one positional field requires a trailing comma.`
- **Supressões e ordem:** sai também depois de recuperação que salta para o `)` (`(int a b) t` →
  `EXPECTED_TOKEN ')'` no `b` e este no `)`).
- **No DartForge:** sem emissor. `parse_record_type` (`types.rs:233-242`) aceita um posicional sem vírgula.
  Amostras FN `record_type_empty_problems_test.dart:11:37` e `record_type_problems_test.dart:11:37`
  (`(int /* missing trailing comma */ ) r2`). Mudança: contar campos e vírgulas e relatar no `)`; conferir que
  `declaration_type_at` (`statements.rs:945`) aceita `(T) nome` como tipo (não verificado).
- **Exemplos (oráculo vivo 3.6.2):**
```dart
main() {
  (int /* missing trailing comma */ ) r2 = (1, );
}
```
  `record_type_one_positional_no_trailing_comma @45 len 1 (2:37) A record type with exactly one positional field requires a trailing comma.`
  Outros: `(int) x = (1,);` → `@4 len 1`; `(int x) y = (1,);` → `@6 len 1`; `f() { (int) x; }` → `@10 len 1`;
  `typedef R = (int);` → `@16 len 1`; `f((int) p, ({}) q) {}` → `@43 len 1`.


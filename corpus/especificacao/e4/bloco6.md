#### E.4.7 No DartForge — onde está cada decisão

Arquivos em `crates/frontend/src/parser/` (linhas do disco nesta sessão). "=" marca comportamento igual ao fasta.

| decisão | fasta 3.6.2 | DartForge | diferença |
|---|---|---|---|
| modelo de recuperação | nunca falha: tokens sintéticos (`rewriter`), a árvore sempre fecha | `PResult`/`Err(ParseError)` + `synchronize_statement` (`statements.rs:175-220`) | o `Err` pula até o `;`/`}` e perde os comandos e diagnósticos que o fasta ainda leria; é a causa comum das cascatas divergentes |
| despacho de comando | `parseStatementX` `pi:5589` | `parse_statement_inner` `statements.rs:236-284`, `parse_keyword_statement` `:287-367` | `)` `]` `}` EOF e 11 palavras → `erro_statement` em vez de comando de expressão |
| `MISSING_STATEMENT` | AstBuilder `ab:4243` (palavra reservada sozinha) | `mod.rs:583-590`; `statements.rs:447` | emitido para tokens e formas que o fasta não cobre (§E.4.6) |
| sem progresso → `UNEXPECTED_TOKEN` | `pi:5493`, `pi:8635`, `pi:9093` | não existe | falta a trinca `MISSING_IDENTIFIER` + `EXPECTED_TOKEN ';'` + `UNEXPECTED_TOKEN` e a diferença de lexema (`';'` no corpo de função) |
| `;` que falta | `ensureSemicolon` `pi:4293`: token real anterior; sempre continua | `garantir_ponto_e_virgula` `mod.rs:542-549` (posição =) | só continua se o seguinte é identificador, palavra, `}`, `@` ou EOF; senão `Err` |
| `)` que falta | `ensureCloseParen` `pi:4234`: com `)` sintético não relata (erro do scanner, reposicionado) | `garantir_fecha_parenteses` `mod.rs:566-578` | com `)` casado: =; sem ele: `Err` com `EXPECTED_TOKEN` no token corrente (mesma posição do erro do scanner movido, mas a leitura para) |
| declaração × expressão | `computeType` + `looksLikeLocalFunction` `pi:8064-8264` | `parse_identifier_statement` `statements.rs:371-398`, `declaration_type_at` `:945`, `looks_like_local_function` `:890`, `conditional_after_question` `:1044` | lookahead próprio; equivalência caso a caso não verificada. `MISSING_CONST_FINAL_VAR_OR_TYPE` em `:477` vira `Err` |
| rótulos | `pi:5763` | `at_label` `statements.rs:884-886`, laço `:239-253` | = |
| `LoopState` | `pi:302`, `pi:377-381` | não existe (`statements.rs:298-317`) | três códigos sem emissor (§E.4.6) |
| `for` | `pi:8335-8570` | `parse_for_header` `statements.rs:70-169`, `parse_classic_for_rest` `:657-680` | `for (var <padrão> = e; …)` recusado (`:85-87`, ver §FOR); sem `COLON_IN_PLACE_OF_IN`, `INITIALIZED_VARIABLE_IN_FOR_EACH`, `INVALID_AWAIT_IN_FOR`, `UNEXPECTED_TOKEN` de sobra, `EXTRANEOUS_MODIFIER` de `late`; sem `(` sintético |
| `if`/`while`/`do` | `ensureParenthesizedCondition` `pi:6729` (`()` sintético) | `statements.rs:598-702` | sem `(`: `Err` em `expect_op`; `do` sem `while`: `Err` |
| `switch` comando | `parseSwitchBlock` `pi:8976` | `parse_switch` `statements.rs:709-734`, `parse_switch_cases` `:736-779` | `SWITCH_HAS_*` = (`:748-752`); sem `{`: `EXPECTED_TOKEN '{'` no token corrente em vez de `EXPECTED_BODY` no anterior (`:716`); nem `case` nem `default`: `EXPECTED_CASE_OR_DEFAULT` (`:767`) em vez de `EXPECTED_TOKEN 'case'`; `:` que falta: `Err` (`:761`, `:764`) em vez de `:` sintético |
| `try` | `pi:8819` | `parse_try` `statements.rs:782-828` | `MISSING_CATCH_OR_FINALLY` no token **corrente** (`:818`) em vez do `try`, e com `Err`; sem `CATCH_SYNTAX*` (`:831-843`) |
| `assert` | `pi:9131` | `parse_assert` `statements.rs:855-879` | sobra antes do `)` = (`:867-875`); sem `(`: `Err`; `assert` em expressão: sem ramo (`expressions.rs:1305`) |
| laço de precedência | `pi:5921` | `parse_binary_rest` `expressions.rs:579-649` | `EQUALITY_CANNOT_BE_EQUALITY_OPERAND` = (`:604-613`); operador por extenso = (`:587-597`, `:702`); `is`/`as` encadeados = (`:685-694`) |
| primária | `parsePrimary` `pi:6547` | `parse_primary` `expressions.rs:1178-1307` | `_ => erro_identificador` (`:1305`, `mod.rs:594-600`): toda palavra reservada dá `EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD` + `Err` — sem o ramo `return` (`UNEXPECTED_TOKEN`), sem distinguir `looksLikeStatementStart`/`as`/`is` (`MISSING_IDENTIFIER` sem consumir), sem identificador sintético, sem função nomeada, sem `assert(…)` |
| `super` | `pi:6874`; AstBuilder `reportErrorIfSuper` em 19 pontos | `expressions.rs:1215-1223`; `conferir_super_solto` `:888-893` | `?.` =; `MISSING_ASSIGNABLE_SELECTOR` só no comando de expressão (`statements.rs:419`) e na atribuição (`expressions.rs:878`) |
| argumentos | `parseArgumentsRest` `pi:7783` | `parse_arguments` `expressions.rs:234-274` | vírgula que falta = (`:257-260`); nomeado com `:` sem nome: sem recuperação |
| record/parêntese literal | `pi:6744` | `parse_parenthesized_or_record` `expressions.rs:1515-1550` | sem `EMPTY_RECORD_LITERAL_WITH_COMMA` nem `RECORD_LITERAL_ONE_POSITIONAL_NO_TRAILING_COMMA` |
| literais de coleção | `pi:6917`, `pi:6990` | `parse_collection_elements` `expressions.rs:1384` | `EXPECTED_ELSE_OR_COMMA` sem emissor; recuperação de vírgula não verificada |
| `new` + literal | `pi:7246` | `expressions.rs:1245-1286` | = |
| inicializadores | expressão + `buildInitializer` (`pi:4052`, `ab:638`) | `parse_initializer` `declarations.rs:2924-3008`, `inicializador_sem_atribuicao` `:3022` | estrutural; sem `INVALID_SUPER/THIS_IN_INITIALIZER`, `INVALID_INITIALIZER`, `FIELD_INITIALIZED_OUTSIDE_DECLARING_CLASS` |
| padrão constante | `pi:5930-5971`, `pi:6386-6425`, `pi:6553-6620` | `patterns.rs:369-470`, `:513-616` | erros `INVALID_CONSTANT_*` portados; fim do `case` com `Err` no `:` que falta (`statements.rs:761`); `void fun` como padrão de variável: não verificado |
| `switch` expressão | `pi:10355` | `parse_switch_expression` `expressions.rs:1652` | `default`, `case`, `:`, `;` = (`:1667-1700`); sem `()` sintético na condição |
| tipo record | `pi:1604-1749` | `parse_record_type` `types.rs:207-253` | nenhum dos três erros de tipo record |
| metadata | `pi:1331`, `pi:7718` | `parse_metadata` `declarations.rs:698-732` | sem `…_UNINSTANTIATED` nem `ANNOTATION_SPACE_BEFORE_PARENTHESIS` |
| versão < 3.0 | `allowPatterns` | não há chave no parser (nenhum uso de `Feature::Patterns` em `parser/*.rs`) | `case` sempre lê padrão (§E `expected_token`: "fora") |
| duplicatas | `Set<AnalysisError>` | `erro_unico` `mod.rs:526-530`, só onde chamado | sem deduplicação geral |

Ordem sugerida de implementação (cada passo com placar antes/depois): (1) `LoopState` (três códigos, sem tocar
na recuperação); (2) os dois erros de tipo record e os dois de metadata (locais, sem `Err`); (3) ramo `return`
e função nomeada em `parse_primary`; (4) inicializador como expressão + classificação; (5) comando de
expressão como recuperação universal com identificador/`;` sintéticos e a regra "sem progresso" — é o que
muda as cascatas de `missing_statement`/`unexpected_token`/`expected_token` e deve ser medido contra o
corpus inteiro.

#### E.4.8 Correções ao texto existente (§E, rodada 3)

1. `missing_statement`: §E diz "`parseStatement` → `ExpectedStatement`". O parser não emite; é o
   `AstBuilder.handleExpressionStatement` (`ab:4243-4248`), só para palavra reservada sozinha como comando.
2. Cauda, `annotation_with_type_arguments_uninstantiated`: §E aponta `ast_builder.dart` e "no `<…>`". É o
   parser (`pi:1346-1349`), no **último token lido** (o `>` final ou o nome depois do `.`).
3. Cauda, `empty_record_type_named_fields_list`: §E diz "no `{`". É no **`}`** (`pi:1741-1745`; oráculo
   `({}) r` → `@2`).
4. `missing_identifier`, "EOF len 0→1": no oráculo vivo o length no EOF é **0**; length 1 no fim do arquivo é
   só dos erros do scanner.
5. `unexpected_token`: "`ensureColon` em `case … :`" — o `ensureColon` emite `EXPECTED_TOKEN ':'`; o
   `UNEXPECTED_TOKEN` do `:` que sobra é do laço "sem progresso" de `parseStatementsInSwitchCase` (`pi:9093`).
   Linhas exatas dos relatos: `pi:7923` (`is`/`as`), `pi:9171` (`assert`), `pi:10378` (`case` em `switch` expressão).
6. `expected_token`, "`new C(;` … `ensureCloseParen` o move e o relato sai no token corrente
   (`parser_impl.dart:4239-4245`)": nesse ramo o `ensureCloseParen` **não relata** (`pi:4239-4243`); o diagnóstico
   é o `ScannerErrorCode.EXPECTED_TOKEN` do `UnmatchedToken`, cuja posição é lida depois do movimento
   (`fe:scanner/errors.dart:72`, relato em `pi:440`).
7. `expected_token`, "`case void fun() {}:` … padrão constante recupera até a igualdade": o padrão ali é o de
   **variável** `void fun` (passo 6 de `parsePrimaryPattern`), não constante.

Não verificado nesta parte: `UnexpectedModifierInNonNnbd` (não exercitado); o ponto de emissão de `Unexpected text 'const'` em
`enum const E3;` no parser 3.13; do lado DartForge, a equivalência do lookahead de declaração local, a
recuperação de vírgula em literais e a leitura de `(T) nome` como tipo record.

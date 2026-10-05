#### E.4.4 Padrões (`pi:9600-10460`)

```
parsePattern(token, ctx, precedence = 1):                                    pi:9600
  token = parsePrimaryPattern(token, ctx)
  laço: tokenLevel = _computePrecedence(next, forPattern: true)   (`?` vale SELECTOR)
        tokenLevel < precedence → fim
        'as'  → [!isLastPatternAllowedInsideUnaryPattern → INVALID_INSIDE_UNARY_PATTERN do início ao último token]
                computeTypeAfterIsOrAs + ensureTypeNotVoid; handleCastPattern
        '!' / '?' → [idem] handleNullAssertPattern / handleNullCheckPattern
        '&&' / '||' → parsePattern(next, ctx, precedence: tokenLevel + 1)
        outro operador → fim                      (não há erro aqui: quem reclama é o chamador)
        depois de as/!/?: isLastPatternAllowedInsideUnaryPattern = false

parsePrimaryPattern(token, ctx):                                              pi:9692
  1. `<…>`? + `[`/`[]` → lista;  `<…>`? + `{` → mapa
  2. `var` / `final` → parseVariablePattern
  3. `(`: se depois do `)` casado vem identificador (ou `?` identificador) e
         computeVariablePatternType dá tipo record não recuperado → variável com tipo record;
         senão `()` → record vazio; senão parseParenthesizedPatternOrRecordPattern
  4. `const` → parsePrecedenceExpression(const, EQUALITY_PRECEDENCE, cascata: não, explicit)
  5. operador relacional ou de igualdade → parsePrecedenceExpression(op, BITWISE_OR_PRECEDENCE, …, none)
         → handleRelationalPattern; isLastPatternAllowedInsideUnaryPattern = false
  6. computeVariablePatternType(token) != noType  (tipo seguido de nome; `T as`/`T when` → noType,
         fe:parser/type_info.dart:357-376) → parseVariablePattern(typeInfo)
  7. identificador [`.` identificador] + `<…>`? + `(` → padrão de objeto
     identificador sozinho: contexto irrefutável ou nome `_` → variável;
         nome em illegalPatternIdentifiers → ILLEGAL_PATTERN_IDENTIFIER_NAME; segue como constante
  8. constante implícita: parsePrecedenceExpression(token, EQUALITY_PRECEDENCE, cascata: não, implicit)
```

**Padrão constante** — o que o laço de expressão faz com `ConstantPatternContext != none`:
- `_parsePrecedenceExpressionLoop` (`pi:5930-5971`): `!` pós-fixo, `!?` e `as` devolvem o controle ao padrão. Um
  operador com `EQUALITY (7) <= tokenLevel < SELECTOR (17)` é relatado **uma vez** e o contexto zera:
  `explicit` → `INVALID_CONSTANT_CONST_PREFIX` no **último token lido**; `implicit`/`numericLiteralOnly` com
  nível ≤ multiplicativo → `INVALID_CONSTANT_PATTERN_BINARY` no operador (arg = lexema); `++`/`--` pós-fixos →
  nada. A expressão binária é lida inteira mesmo assim.
- Operadores de nível **< 7** (`??`, `=`, `..`, `?` de condicional) **não são lidos**: a constante acaba antes
  deles; `&&`/`||` são operadores de padrão; o resto encerra o padrão e o chamador (`ensureColon`,
  `ensureCloseParen`, `ensureFunctionArrow`) relata.
- `parseUnaryExpression` (`pi:6386-6425`): `!`/`~` → `INVALID_CONSTANT_PATTERN_UNARY` no operador; `-` em
  `explicit` → `INVALID_CONSTANT_CONST_PREFIX` no `-`; `-` em `implicit` → operando em `numericLiteralOnly`
  (identificador, string, símbolo, `true`/`false`/`null` → `INVALID_CONSTANT_PATTERN_NEGATION` no operando,
  `pi:6553-6620`); **`++`/`--` prefixos: nenhum erro** (o operando é lido sem contexto).
- `parsePrimary` em `explicit`: literal numérico/string/símbolo/bool/null → `INVALID_CONSTANT_CONST_PREFIX` no
  literal; `const` → `INVALID_CONSTANT_PATTERN_DUPLICATE_CONST` no segundo `const`; `parseSend` em `explicit` cujo
  token seguinte não é `.`, `(`, `<` → `INVALID_CONSTANT_CONST_PREFIX` no identificador (`pi:7687-7698`);
  argumentos de tipo sem `(` → `INVALID_CONSTANT_PATTERN_GENERIC` no `<`; `()` →
  `INVALID_CONSTANT_PATTERN_EMPTY_RECORD_LITERAL` no `(` (`pi:6813-6816`).

Casos de `case` (3.x, `allowPatterns`; oráculo vivo):

| entrada (`f(x) { switch (x) { … } }`) | caminho | diagnósticos |
|---|---|---|
| `case 1 ?? 2:` | constante `1`; `??` (nível 4) encerra; `ensureColon` no `??`; corpo: comando `?? 2` (identificador que falta no `??`, `;` que falta no `2`); `:` sem progresso | `expected_token @27 len 2 ':'`, `missing_identifier @27 len 2`, `expected_token @30 len 1 ';'`, `missing_identifier @31 len 1`, `unexpected_token @31 len 1 Unexpected text ':'.` |
| `case ++a:` | passo 8: `++` prefixo sem erro | nenhum sintático (`constant_pattern_with_non_constant_expression @30 len 1`) |
| `case void fun() {}:` | passo 6: `void fun` é **padrão de variável** de tipo `void`; `ensureColon` no `(`; corpo: `() {}` (expressão de função) com `;` que falta no `}`; `:` sem progresso | `expected_token @33 len 1 ':'`, `expected_token @37 len 1 ';'`, `missing_identifier @38 len 1`, `unexpected_token @38 len 1 Unexpected text ':'.` |
| `case const void fun() {}:` | passo 4: `parsePrimary(void)` → função nomeada | `named_function_expression @36 len 3` (só) |
| `case assert(false):` | passo 8: `parsePrimary` → `parseAssert(Expression)`; mensagem descartada | nenhum sintático (`undefined_identifier @25 len 6`) |
| `case 1 + 2: case -a: case !true: case const 1: case const -1: case const const A():` | laço / unário / primário | `invalid_constant_pattern_binary @27 len 1`, `invalid_constant_pattern_negation @38 len 1` (no `a`), `invalid_constant_pattern_unary @46 len 1`, `invalid_constant_const_prefix @64 len 1` (no `1`), `invalid_constant_const_prefix @78 len 1` (no `-`), `invalid_constant_pattern_duplicate_const @93 len 5` |
| `case var a? as int:` | `as` depois de `?` | `invalid_inside_unary_pattern @42 len 6` (de `var` ao `?`) |
| `case when: case var when:` | passo 7 / `parseVariablePattern` | `illegal_pattern_identifier_name @25 len 4`, `illegal_pattern_variable_name @40 len 4` |
| `var (var a, b) = (1, 2); (var c, d) = (1, 2);` | `parseVariablePattern` (`pi:9899-9926`) | `variable_pattern_keyword_in_declaration_context @11 len 3`, `pattern_assignment_declares_variable @36 len 1` |

**Versão < 3.0** (`allowPatterns = featureSet.isEnabled(Feature.patterns)`, `an:generated/parser.dart:43`): o `case`
lê **expressão** (`pi:9014-9018`); `if (… case …)` não existe (`pi:6838`); `switch` não é expressão (`pi:6646`);
`var (…) =` não é declaração por padrão (`pi:8085`); `{…} =` não é atribuição por padrão (`pi:5604`, `pi:5828`).
Medido com `// @dart=2.19`:
- `case 1 ?? 2: break; case void fun() {}: break; case assert(false): break; case ++x: break;` → só
  `named_function_expression @64 len 3` (sintático); o `:` fecha cada `case` normalmente.
- `case var a: break;` → `missing_identifier @39 len 3` e `expected_token @39 len 3 ':'` (no `var`),
  `expected_token @43 len 1 ';'` (no `a`), `missing_identifier @44 len 1`, `unexpected_token @44 len 1 ':'`.
- `if (x case 1) {}` → `expected_token @27 len 4 ')'` (no `case`); `var y = switch (x) { _ => 1 };` →
  `expected_token @44 len 1 ';'` (no `=`), `missing_identifier @46 len 6` (no `switch`), e o `switch` vira
  comando: `expected_token @59 len 1 'case'` (no `_`).
- Tipos e literais record são lidos pelo parser em qualquer versão; o AstBuilder troca por
  `EXPERIMENT_NOT_ENABLED` (`records`) no `(` (`ab:2960-2964`, `ab:3008-3012`): `(int, int) r = (1, 2);` →
  `experiment_not_enabled @20 len 1` e `@35 len 1`.
- O `LoopState` não depende da versão: `switch (x) { case 1: continue; }` → `continue_without_label_in_case` igual.

**`switch` expressão** (`pi:10355-10460`): `default` → `DEFAULT_IN_SWITCH_EXPRESSION` (vale `_`); `case` →
`UNEXPECTED_TOKEN` no `case` (pulado); depois do padrão/guarda: `:` → `ExpectedButGot '=>'` no `:` (aceito
como seta); senão `ensureFunctionArrow`; depois do corpo: `,`; `;` → `ExpectedButGot ','` no `;` (aceito);
sem separador: `looksLikePatternStart(next)` → `ExpectedButGot ','` em `next` + vírgula sintética; senão procura
a próxima `,`/`;` do bloco: achou → `ExpectedButGot ','` em `next` e pula até lá; não achou →
`ExpectedButGot '}'` em `next` e pula para o `}`. Sem `{` → `EXPECTED_SWITCH_EXPRESSION_BODY`. Não mexe em
`loopState`; `mayParseFunctionExpressions` é `false` no padrão/guarda e `true` no corpo.
Medidos: `switch (x) { case 1 => 2, default => 3 }` → `unexpected_token @28 len 4 'case'`,
`default_in_switch_expression @41 len 7`; `{ 1 : 2; _ => 3 }` → `expected_token @30 len 1 '=>'`,
`expected_token @33 len 1 ','`; `{ 1 => 2 _ => 3 }` → `expected_token @35 len 1 ','`; `{ 1 => 2 + }` →
`missing_identifier @37 len 1`.

#### E.4.5 Tipos record e metadata

##### `parseRecordType` (`pi:1604-1694`), `parseRecordTypeField` (`pi:1696`), `parseRecordTypeNamedFields` (`pi:1713`)

Chamado por `ComplexTypeInfo.parseType` depois que `computeType` decidiu que o `(` abre um tipo record (a
decisão é de `type_info.dart`, fora desta parte). Laço:

```
')' → fim.   primeiro token ',' seguido de ')' → illegalTrailingComma; fim
'{' → parseRecordTypeNamedFields; ensureCloseParen; fim
campo: parseMetadataStar; computeType(required: true).ensureTypeOrVoid; nome opcional (obrigatório nos nomeados)
depois do campo:  ',' → sawComma, continua;  ')' → fim
   senão: ')' sintético → move;  identificador + identificador → ExpectedButGot ',' + vírgula sintética, continua;
          senão ensureCloseParen
fim:  parameterCount == 0 && illegalTrailingComma → EMPTY_RECORD_TYPE_WITH_COMMA na ','            pi:1673-1676
      parameterCount == 1 && !hasNamedFields && !sawComma
                          → RECORD_TYPE_ONE_POSITIONAL_NO_TRAILING_COMMA no ')'                    pi:1677-1680
nomeados: laço até '}'; campo sem ',' nem '}' → ExpectedButGot '}' e salta para o '}' casado       pi:1728-1735
          parameterCount == 0 → EMPTY_RECORD_TYPE_NAMED_FIELDS_LIST no '}'                         pi:1743-1745
```

`parameterCount` conta o grupo `{…}` como 1: `({})` tem `parameterCount == 1`, `hasNamedFields` → só o erro dos
nomeados. Literal (`parseParenthesizedExpressionOrRecordLiteral`, `pi:6744-6824`): `(,)` →
`EMPTY_RECORD_LITERAL_WITH_COMMA` na `,`; `RECORD_LITERAL_ONE_POSITIONAL_NO_TRAILING_COMMA` só com `const (e)`
(`wasRecord` nasce `true` só com `const`; sem `const`, `(e)` é parêntese), no `)`.

| entrada | diagnósticos |
|---|---|
| `(,) r = ();` | `empty_record_type_with_comma @1 len 1` |
| `(int x) y = (1,);` | `record_type_one_positional_no_trailing_comma @6 len 1` |
| `f() { (int, int a, {int b}) r; ({int}) s; (int a b) t; }` | `missing_identifier @36 len 1` (no `}`: nomeado sem nome); `expected_token @49 len 1 ')'` (no `b`) e `record_type_one_positional_no_trailing_comma @50 len 1` |
| `var r = (1); var s = (,); var t = const (1);` | `empty_record_literal_with_comma @28 len 1`; `record_literal_one_positional_no_trailing_comma @48 len 1` |
| `var s = (: 1);` | `missing_identifier @41 len 1` (no `:`) |

##### `parseMetadata` (`pi:1331-1353`) e `parseArgumentsOptMetadata` (`pi:7718-7752`)

```
'@' ensureIdentifier(metadataReference); parseQualifiedRestOpt(metadataContinuation)   (no máximo UM `.id`)
hasTypeArguments = next == '<';  computeTypeParamOrArg(token).parseArguments
'.' → ensureIdentifier(metadataContinuationAfterTypeArguments)        (aceita `new`)
hasTypeArguments && next != '(' → ANNOTATION_WITH_TYPE_ARGUMENTS_UNINSTANTIATED em `token`        pi:1346-1349
parseArgumentsOptMetadata(token, hasTypeArguments):
   next != '(' → sem argumentos
   '(' colado (token.charEnd == next.charOffset) → parseArguments
   '(' separado:  hasTypeArguments → ANNOTATION_SPACE_BEFORE_PARENTHESIS no '(' e lê argumentos   pi:7729-7733
                  o token depois do ')' casado é `class` ou `enum` → idem                         pi:7739-7746
                  senão → sem argumentos (o '(' é de um tipo record/declaração seguinte)
AstBuilder.endMetadata (ab:2504-2531): typeArguments != null e generic_metadata desligado (< 2.14)
                  → EXPERIMENT_NOT_ENABLED no '<'
```

`ANNOTATION_WITH_TYPE_ARGUMENTS` (`MetadataTypeArguments`, índice 91) **não é emitido** pelo 3.6.2: a mensagem
não aparece em `parser_impl.dart` nem em `ast_builder.dart` (só na tabela de índices).

| entrada | diagnósticos sintáticos |
|---|---|
| `@A () class C {}` | `annotation_space_before_parenthesis @26 len 1` (no `(`) |
| `@A<int> () class C {}` | `annotation_space_before_parenthesis @34 len 1` (sem `…_uninstantiated`: há `(`) |
| `@A` + quebra + `(int, int) f() => (1, 2);` | nenhum (o `(` é do tipo de retorno) |
| `@A.b.c.d class C {}` | `expected_executable @6 len 1` (no 3º `.`), `missing_const_final_var_or_type @7 len 1`, `expected_token @7 len 1 ';'` |
| `@ class C {}` | `missing_identifier @2 len 5` (no `class`) |
| `// @dart=2.12` … `@A<int>() class C {}` | `experiment_not_enabled @42 len 1` (`generic-metadata`, no `<`) |


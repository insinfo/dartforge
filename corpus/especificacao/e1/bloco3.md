
#### E.1.10 Códigos da família

Estado do DartForge no disco (vale para todos os códigos abaixo): `lexer::lex`
(`crates/frontend/src/lexer.rs:50-60`) devolve `Result<Vec<Token>, Diagnostic>` e **para no primeiro erro
léxico**; `parse_lexed_com` (`crates/frontend/src/parser/mod.rs:144-159`) devolve, nesse caso, uma unidade
**vazia** com esse único diagnóstico. Não existe `lex_recuperando` no disco (corrige §SCANNER da rodada 3,
ver E.1.13). O lexer não tem pilha de grupos nem tokens sintéticos.

##### `unterminated_string_literal` (perda 6: FN 6)
- **Emissão:** scanner — `unterminatedString` (`fe:scanner/abstract_scanner.dart:2055-2067`), chamado por
  `tokenizeSingleLineString` (:1841-1846), `tokenizeSingleLineRawString` (:1910-1914, :1920-1923),
  `tokenizeMultiLineRawString` (:1960-1963) e `tokenizeMultiLineString` (:2014-2017); `UnterminatedString`
  na cabeça da fila; relato no fim de `parseUnit` por `AstBuilder.handleErrorToken`
  (`an:fasta/ast_builder.dart:4206`) → `translateErrorToken` (`fe:scanner/errors.dart:32-37`).
- **Condição exata:** string de uma linha (crua ou não): LF, CR ou EOF antes da aspa de fecho — inclusive
  logo depois de uma `\` (a `\` consome um caractere, mas a quebra é testada em seguida, :1832-1846);
  multilinha: EOF antes das três aspas. Dentro de `${…}` não há erro de string enquanto a expressão não
  fecha (a interpolação atravessa linhas); no EOF dentro de `${`, a string externa também é relatada.
- **Posição:** `endOffset - 1`, `endOffset` = offset do LF/CR/EOF onde a varredura parou; length 1. É o
  último caractere antes da quebra (ou o último do arquivo), **não** a aspa de abertura; pode ser um
  caractere de outra construção (o `}` da interpolação, exemplo a02) ou o LF anterior (multilinha no EOF,
  exemplo a26; string aberta no fim de `${` que engoliu linhas, exemplo a45). Offsets em UTF-16 (exemplo
  b17). Sem `_isAtEnd`.
- **Mensagem:** `Unterminated string literal.` (`fe:scanner/errors.dart:186-188`), sem argumentos.
- **Supressões e ordem:** nada suprime. Um erro por string aberta. Token sintético STRING com a aspa
  acrescentada → o parser segue; cascata típica: `expected_token` (`;`) do parser **no token da string**
  (offset da aspa de abertura, length = texto real), e, se a string aberta estava dentro de `(`, o `)`
  do scanner onde o grupo fechar (exemplo a24). `'abc\` + quebra dá também
  `invalid_unicode_escape_started` no mesmo offset. Na lista, vem depois dos erros do parser.
- **No DartForge:** `lexer.rs:343-361` (`string_body`) detecta fim de arquivo (:344-346) e LF em string de
  uma linha (:359-361) e devolve `Err(string_nao_terminada())`; a posição (`lexer.rs:105-115`: último
  caractere antes do LF, recuando o CR de um CRLF e alinhando a fronteira de caractere) já é a do fasta.
  Diferenças: (1) aborta — as 6 FN são o 2º erro em diante de cada arquivo
  (`string/bad_raw_string_test.dart:13:1, 17:6, 22:1, 32:6, 37:1`; `string/escape4_test.dart:20:3`); os 2
  acertos são o primeiro erro de cada um; (2) CR sozinho não termina a string (:359 só testa `\n`);
  (3) não há token sintético. Mudança: modo de recuperação que emite `Str`/`StrEnd` até a quebra (sem aspa
  de fecho), registra o diagnóstico e segue na linha seguinte.
- **Exemplos (oráculo vivo 3.6.2):** a01, a24, a26, a27, a35, a49, a55, b07, b16, b17 em E.1.11.

##### `unsupported_operator` (perda 5: FN 5)
- **Emissão:** scanner — `tokenizeEquals` (:1182-1185) e `tokenizeExclamation` (:1157-1160):
  `appendPrecedenceToken(EQ_EQ_EQ | BANG_EQ_EQ)` + `prependErrorToken(UnsupportedOperator(tail,
  tokenStart))`; relato por `translateErrorToken` (`fe:scanner/errors.dart:66-68`).
- **Condição exata:** três caracteres `===` ou `!==` seguidos, em qualquer contexto fora de string e
  comentário. O quarto `=` é outro token.
- **Posição:** início do operador, length **1** (não 3).
- **Mensagem:** `The '{0}' operator is not supported.`, `{0}` = lexema do token (`===` ou `!==`).
- **Supressões e ordem:** o token tem precedência de igualdade (`fe:scanner/token.dart:1325`, :1383) e o
  parser o trata como operador binário: `a === b` e `operator ===(x)` **não** dão erro sintático (exemplos
  a09, a48). O erro é o único diagnóstico.
- **No DartForge:** `lexer.rs:463-517` (tabela de `operator`) não tem `===`/`!==`: saem `==` + `=` e
  `!=` + `=`, e o parser relata outra coisa — FP `missing_identifier` em `operator/unsupported_test.dart:12:19,
  19:19, 32:17, 39:17` e FP `expected_token` (`(`) em `operator/operator7_test.dart:8:14`; FN nas 5 amostras
  (`operator/operator7_test.dart:8:12`, `operator/unsupported_test.dart:12:17, 19:17, 32:15, 39:15`).
  Mudança: dois operadores de três caracteres com a precedência de `==`/`!=` (podem reutilizar
  `Op::EqEq`/`Op::BangEq` com span de 3) + diagnóstico de length 1 no início; aceitar o token depois de
  `operator`.
- **Exemplos:** a09, a48, b20.

##### `missing_digit` (perda 2: FN 2)
- **Emissão:** scanner — `tokenizeFractionPart` (:1427-1433): `UnterminatedToken(messageMissingExponent,
  tokenStart, stringOffset)`; `translateErrorToken` (`fe:scanner/errors.dart:46-50`).
- **Condição exata:** número (inteiro ou com fração) seguido de `e`/`E`, sinal opcional, e **nenhum**
  dígito de expoente. `1.e` não entra (é `1` `.` `e`).
- **Posição:** `endOffset - 1` = último caractere lido do número: o `e` (`1e;`) ou o sinal (`1e+;`);
  length 1.
- **Mensagem:** `Decimal digit expected.`
- **Supressões e ordem:** token DOUBLE sintético (lexema + `0`, length real) — nenhum outro erro
  (exemplos a06, a07, a47); no EOF, o `;` que falta sai no token do número (exemplo a34).
- **No DartForge:** `lexer.rs:268-274` e :304: sem dígito de expoente (`expoente_valido`, :284-296) o
  número termina antes do `e` e o `e` vira identificador — FN `double/invalid_test.dart:8:7`,
  `number/identifier_test.dart:48:4`, com FP `expected_token` (`;`) em `8:3`/`48:3` e
  `undefined_identifier` `'e'`. Mudança: consumir `e` e sinal como o fasta, token `Double` (valor do
  lexema + `0`), diagnóstico no último caractere.
- **Exemplos:** a06, a07, a34, a47.

##### `missing_hex_digit` (perda 1: FN 1)
- **Emissão:** scanner — `tokenizeHex` (:1330-1337): `UnterminatedToken(messageExpectedHexDigit, start,
  stringOffset)` + HEXADECIMAL sintético; `translateErrorToken` (`fe:scanner/errors.dart:52-56`).
- **Condição exata:** `0x`/`0X` sem nenhum dígito hexadecimal depois (os `_` não contam).
- **Posição:** `endOffset - 1`: o `x` (`0x;`) ou o último `_` (`0x_;`); length 1.
- **Mensagem:** `Hexadecimal digit expected.`
- **Supressões e ordem:** sozinho em `0x;` (exemplo a05). `0xg` → mais os erros do parser para o
  identificador `g` (exemplo a51). `0x_` → mais `unexpected_separator_in_number` e
  `integer_literal_out_of_range` (exemplo b10).
- **No DartForge:** `lexer.rs:251` só entra no ramo hex se o 3º caractere é dígito hex ou `_`; `0x;` sai
  `Int(0)` + identificador `x` — FN `number/identifier_test.dart:13:4`, FP `expected_token` `13:3` e
  `undefined_identifier` `'x'`. Mudança: `0x`/`0X` sempre abre hex; sem dígito, token `Int` de valor 0 e
  diagnóstico no último caractere.
- **Exemplos:** a05, a33, a51, a57, b10.

##### `illegal_character` (perda 1: pos 1)
- **Emissão:** scanner — `unexpected` (:2020-2048), a partir de `bigSwitch` (:993-999) e de
  `tokenizeIdentifier` (:1778-1779); `translateErrorToken` (`fe:scanner/errors.dart:58-61`).
- **Condição exata:** fora de string e comentário, unidade UTF-16 que não é ASCII imprimível, TAB, LF ou
  CR: controles `< 0x1f`, `0x1f`, `0x7f`, tudo `≥ 0x80` (tabela de E.1.6). `U+FFFD` derruba o analyzer.
- **Posição:** offset da unidade UTF-16, length 1. Par substituto: dois erros, offsets `n` e `n + 1`.
- **Mensagem:** `Illegal character '{0}'.`, `{0}` = valor **decimal** da unidade UTF-16 (`248` para `ø`,
  `55357` e `56832` para U+1F600).
- **Supressões e ordem:** espaço não ASCII e controle são pulados sem token; os demais viram parte de um
  identificador (fundem com o identificador vizinho), então o parser não acrescenta erro sintático; a
  resolução pode acrescentar `undefined_identifier` com o texto original.
- **No DartForge:** `lexer.rs:192-198` (byte `≥ 0x80`) e :525-527 (resto) → `caractere_ilegal`
  (:98-101): código e argumento certos para o BMP. Diferenças: (1) o span é `[inicio, inicio + 1)` em
  **bytes** (:92-94); num caractere de 2+ bytes o fim cai no meio do caractere e a conversão para UTF-16
  (`crates/paridade/src/json.rs:108-110`, recuo até a fronteira) dá length 0 — é a amostra "posição" de
  `illegal_character/IllegalCharacter__nonAsciiIdentifier.dart:1:8` (mesma linha:coluna, comprimento
  diferente); (2) aborta a unidade; (3) fora do BMP relata um erro com o ponto de código (`128512`) em vez
  de dois com as metades; (4) `` ` `` e `\` são ilegais aqui e tokens lá; FF (`0x0C`) é espaço aqui
  (`is_ascii_whitespace`, :129) e ilegal lá; (5) não funde o identificador. Mudança: span do caractere
  inteiro (`inicio + ch.len_utf8()`); para fora do BMP, dois diagnósticos (o segundo offset não é
  representável em bytes: exige decisão no `Span`/conversor — hoje sem amostra no placar); recuperação
  com fusão de identificador.
- **Exemplos:** a15, a16, a17, a18, a19, a37, a42, b11, c03.

##### `invalid_unicode_escape_started` (perda 1: FN 1)
- **Emissão:** **não é do scanner**: `unescapeCodeUnits` (`fe:parser/quote.dart:199-205`) via
  `AstBuilder.endLiteralString` (`an:fasta/ast_builder.dart:2390`) e `StackListener.handleUnescapeError`
  (`fe:parser/stack_listener.dart:413`); fase de parse (AstBuilder).
- **Condição exata:** trecho de string não crua cujo **último** caractere (depois de tirar as aspas) é
  uma `\`. Como o scanner sempre consome o caractere depois da `\`, só acontece em string **não
  terminada**: `'…\` + LF/CR/EOF (a aspa sintética de fecho é cortada e sobra a `\` no fim).
- **Posição:** `token.charOffset + i`, `i` = índice depois da `\` no trecho sem a aspa de abertura;
  length 1. Com aspa simples: a própria `\`, o mesmo offset do `unterminated_string_literal`.
- **Mensagem:** `The string '\' can't stand alone.` (correção: `Try adding another backslash (\) to escape
  the '\'.`).
- **Supressões e ordem:** sempre acompanhado de `unterminated_string_literal` no mesmo offset (exemplos
  a24, a40, a58).
- **No DartForge:** `crates/frontend/src/parser/expressions.rs:98-170` (`erro_de_escape`, porte de
  `unescapeCodeUnits`; este caso em :109-112) chamado de :1888-1909. FN
  `string/escape4_test.dart:11:23`: o lexer aborta em `unterminated_string_literal` antes do parser. Com a
  recuperação do lexer, o trecho `Str` sem fecho precisa chegar a `push_string_text` com o conteúdo
  terminando na `\`. Divergência de posição dos demais escapes (não é deste código): `expressions.rs`
  relata sempre na `\` (`base + begin - 1`, :1898-1901); o analyzer só coincide quando o corte inicial é
  de 1 caractere (E.1.5) — em `'''…'''` e em trechos depois de interpolação a posição do oráculo é outra.
- **Exemplos:** a24, a40, a58; correlatos a20–a23, a56, a59.

##### `expected_token` — parte do scanner (perda total do código 85; ver a parte do parser)
- **Emissão:** `unmatchedBeginGroup` (:697-747) a partir de `discardBeginGroupUntil` (:648),
  `appendEofToken` (:353) e `discardInterpolation` (:691); `translateErrorToken` (`fe:scanner/errors.dart:71-86`)
  → `ScannerErrorCode.EXPECTED_TOKEN`.
- **Condição exata:** abridor `(`, `[`, `{`, `${` (ou `<` abaixo de outro abridor) descartado por um fecho
  de outro tipo que casa mais abaixo na pilha (opção 1 de E.1.2), ou ainda aberto no EOF, ou aberto no
  EOF dentro de `${`.
- **Posição:** offset do fecho sintético: início do fecho que não casou, ou comprimento do arquivo (além
  do fim), ou — se o parser o moveu (`moveSynthetic`) — o token seguinte ao ponto onde o parser esperava
  o fecho (`new C(;` → no `;`). Length 1.
- **Mensagem:** `Expected to find '{0}'.` com `)`, `]`, `}` ou `>` — mesmo texto do código do parser.
- **Supressões e ordem:** `ensureCloseParen` não relata quando o fecho é sintético (só sai o do scanner);
  `parseArgumentOrIndexStar` relata e move (dois iguais). Vários fechos: mais interno primeiro, todos no
  fim da lista.
- **No DartForge:** só o caso EOF existe: `parser/mod.rs:505-508` (`erro_esperado` com `)`/`]`/`}` no
  `Eof` → código do scanner, length 1). Fecho de outro tipo: `garantir_fecha_parenteses`
  (`parser/mod.rs:566-577`) usa `matching_close` (`parser/mod.rs:701-719`, contagem de profundidade que
  não distingue o tipo do fecho) e relata no token corrente; não há as duas opções de recuperação nem
  fecho sintético. Amostras FN com `)` do scanner movido para o `;`:
  `extra_positional_arguments/ExtraPositionalArguments__constructorIn_e5fb1f6b.dart:3:9`,
  `not_enough_positional_arguments/NotEnoughPositionalArguments__construct_78e49edc.dart:5:9`,
  `new/expression2_test.dart:8:9`, `new/expression3_test.dart:8:12`; `string/escape4_test.dart:27:1`
  (`)` do `print(` fechado pelo `}` da função). Mudança: pilha de grupos no lexer (ou num passo entre
  lexer e parser) que produza, por abridor, o índice do fecho (real ou sintético) e a lista de fechos
  sintéticos com posição; o parser consulta `endGroup` em vez de `matching_close` e "move" o sintético
  nos pontos listados em E.1.2.
- **Exemplos:** a10, a11, a12, a31, a32, a43, a44, a50, b01, b03, b04, b13, b14, b15, c01, c07.

##### `missing_identifier` — parte do scanner (perda total do código 42; ver a parte do parser)
- **Emissão:** `tokenizeInterpolatedIdentifier` (:1891-1896) → `UnterminatedToken(messageUnexpectedDollarInString)`
  → `ScannerErrorCode.MISSING_IDENTIFIER` (`fe:scanner/errors.dart:87-88`).
- **Condição exata:** `$` em string não crua seguido de algo que não é `{`, letra ASCII nem `_`.
- **Posição:** o caractere **depois** do `$` (a aspa em `"$"`, o dígito em `"$1"`, o segundo `$` em
  `"$$a"`), length 1; no EOF, o offset do fim do arquivo.
- **Mensagem:** `Expected an identifier.` (a mensagem de `UNEXPECTED_DOLLAR_IN_STRING` não é usada).
- **Supressões e ordem:** o identificador sintético vazio satisfaz o parser: nenhum outro erro
  (exemplo a25).
- **No DartForge:** `lexer.rs:424-442` (trecho `StrBegin/StrMid(Interp::Ident)` sem token de nome) +
  `parser/expressions.rs:1820-1828` (relata `codigos::scanner::MISSING_IDENTIFIER` no início do token
  seguinte, length 1): equivalente; nenhuma amostra do placar atribuída a este caminho (não verificado
  amostra a amostra).
- **Exemplos:** a25, b18.

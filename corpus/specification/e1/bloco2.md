
#### E.1.4 Strings e interpolação

`tokenizeString(next, start, raw)` (:1795-1814): `start` é o offset da aspa (ou do `r`, via
`tokenizeRawStringKeywordOrIdentifier`, :1712-1721: `r` seguido de aspa). Aspa repetida duas vezes mais
→ multilinha (`tokenizeMultiLineString`, :1966, ou `tokenizeMultiLineRawString`, :1926); aspa repetida
uma vez → string vazia; senão `tokenizeSingleLineString` (:1828) ou `tokenizeSingleLineRawString` (:1902).

```
tokenizeSingleLineString(next, quoteChar, quoteStart):          # :1828-1855
  start = quoteStart
  enquanto next != quoteChar:
    se next == '\': next = advance()                 # o caractere escapado é consumido às cegas…
    senão se next == '$':
      next = tokenizeStringInterpolation(start, asciiOnly); start = scanOffset; continue
    se next é LF, CR ou EOF:                         # …mas uma quebra depois da '\' ainda termina a string
      unterminatedString(quoteChar, quoteStart, start, isMultiLine: false, isRaw: false); devolve next
    next = advance()
  next = advance(); token STRING [start, scanOffset); devolve next
```

- Crua de uma linha (:1902-1924): sem `\` nem `$`; LF, CR ou EOF → `unterminatedString(…, isRaw: true)`.
- Multilinha (:1966-2018) e multilinha crua (:1926-1964): só o EOF termina sem fecho
  (`unterminatedString(…, isMultiLine: true)`); `\` + EOF sai do laço (:1996-1999); cada LF chama
  `lineFeedInMultiline` (:378).
- **`unterminatedString(quoteChar, quoteStart, start, …)`** (:2055-2067):
  `suffix` = a aspa (uma, ou três se multilinha); `prefix` = `r` + suffix se crua;
  `appendSyntheticSubstringToken(STRING, start, asciiOnly, suffix)` → `SyntheticStringToken` cujo **lexema**
  é o texto de `start` até o ponto de parada **mais a aspa sintética**, e cujo `length` é só o texto real
  (`fe:scanner/token.dart:773-791`, `fe:scanner/string_scanner.dart:109-117`); depois
  `errorStart = tokenStart < stringOffset ? tokenStart : quoteStart` e
  `prependErrorToken(UnterminatedString(prefix, errorStart, stringOffset))`
  (`fe:scanner/error_token.dart:179-200`; `endOffset = stringOffset` = offset do LF/CR/EOF).
  O analyzer relata em **`endOffset - 1`** (`fe:scanner/errors.dart:32-37`, sem passar por `_makeError`):
  o último caractere antes da quebra/fim, seja ele qual for — o `;` de `'abc;`, a própria aspa de um `'`
  sozinho na linha, o `}` de uma interpolação fechada na linha (exemplo a02), a `\` de `'abc\`, o LF final
  de uma multilinha sem fecho (exemplo a26). Com CRLF a string para no CR: o erro fica no caractere antes
  do CR (exemplo a55). A linha seguinte é lida normalmente (a string **não** continua).
- O parser recebe um token STRING normal; o que segue costuma dar `expected_token` (`;`) do
  `ensureSemicolon` (`fe:parser/parser_impl.dart:4293-4306`) **no token da string** (offset do token,
  length real: `findPreviousNonZeroLengthToken`, `fe:parser/util.dart:45-57`). Se o token da string tem
  comprimento 0 (resto vazio depois de uma interpolação), `reportRecoverableError`
  (`fe:parser/parser_impl.dart:9429-9433`) avança com `findNonZeroLengthToken` (`fe:parser/util.dart:62-67`)
  até o próximo token não sintético e pode parar no EOF: daí os `expected_token` de **length 0** no fim do
  arquivo (exemplos a02, a03, b16, c08).

**Interpolação** — `tokenizeStringInterpolation(start, asciiOnly)` (:1857-1866): fecha o trecho corrente
como token STRING `[start, offset do $)` (pode ter comprimento 0, e não é sintético), `beginToken()` no
`$`, lê o caractere seguinte:

- `{` → `tokenizeInterpolatedExpression` (:1868-1883): `appendBeginGroup(STRING_INTERPOLATION_EXPRESSION)`
  (token `${`, offset do `$`), depois `bigSwitch` em laço até `$EOF` ou `$STX`. O `}` que casa devolve
  `$STX` (`appendEndGroupInternal`, :424-433); o trecho seguinte da string começa depois dele. Strings,
  comentários e chaves aninham naturalmente (é o mesmo `bigSwitch`). Um caractere `U+0002` literal dentro
  da expressão é confundido com o `$STX` e encerra a interpolação sem fechar o grupo (exemplo b12;
  curiosidade, sem amostra no placar).
- senão → `tokenizeInterpolatedIdentifier` (:1885-1900): token `STRING_INTERPOLATION_IDENTIFIER` (`$`); se
  o caractere é letra ASCII ou `_`: `tokenizeKeywordOrIdentifier(next, allowDollar: false)` — o nome para
  no primeiro `$` e palavras-chave saem como `KeywordToken` (`$this` → expressão `this`; `$class` → o
  parser relata `expected_identifier_but_got_keyword`, exemplo a39). Senão (dígito, aspa, espaço, outro
  `$`, EOF…): `appendSyntheticSubstringToken(IDENTIFIER, scanOffset, true, '')` (identificador sintético
  vazio, comprimento 0, no caractere depois do `$`) e
  `prependErrorToken(UnterminatedToken(messageUnexpectedDollarInString, tokenStart, stringOffset))`, com
  `tokenStart` = offset do caractere **depois** do `$`.
- O parser (`parseSingleLiteralString`, `fe:parser/parser_impl.dart:7549-7579`): para `${`,
  `parseExpression` e, se o token seguinte não é `}`, `ExpectedButGot('}')` **nesse token** e salto para
  `next.endGroup` (:7561-7565) — é erro do **parser**, distinto do `}` do scanner; para `$id`,
  `parseIdentifierExpression` (:7581); depois de cada interpolação, `parseStringPart` (:3258-3267:
  `ExpectedString` + string sintética se o token seguinte não é STRING).

#### E.1.5 Decodificação de escapes (não é o scanner)

O scanner não valida escapes: só pula o caractere depois da `\`. Quem decodifica é o **AstBuilder**,
durante o parse: `endLiteralString` (`an:fasta/ast_builder.dart:2390-2446`) chama `unescapeString`
(string sem interpolação, :2395), `unescapeFirstStringPart` (:2411), `unescape` (trechos do meio, :2420) e
`unescapeLastStringPart` (:2436) de `fe:parser/quote.dart`. `unescape` (:151-182) só chama
`unescapeCodeUnits` se o trecho contém `\` (ou CR, nas multilinhas); strings cruas não decodificam.
O erro sai por `listener.handleUnescapeError(message, location, stringOffset, length)` →
`StackListener.handleUnescapeError` (`fe:parser/stack_listener.dart:413-416`):
`addProblem(message, token.charOffset + stringOffset, length)` → `FastaErrorReporter.reportMessage`
(`an:fasta/error_converter.dart:557`). Como é erro do AstBuilder, entra na lista **na ordem do parse**,
antes dos erros do scanner.

`unescapeCodeUnits(codeUnits, isRaw, location, listener)` (`fe:parser/quote.dart:186-338`), com
`L = codeUnits.length`, `i` = índice corrente, `begin` = índice do `x`/`u` (o caractere depois da `\`):

| situação | mensagem fasta → código | `stringOffset` | `length` | continua? |
|---|---|---|---|---|
| `\` é o último caractere (:200-205) | `InvalidEscapeStarted` → `INVALID_UNICODE_ESCAPE_STARTED` | `i` (= `L`) | 1 | não |
| `\x` com menos de 2 caracteres depois (:231-235) | `InvalidHexEscape` → `INVALID_HEX_ESCAPE` | `begin` | `L + 1 - begin` | não |
| `\x` com não-hex na posição `i` (:239-243) | idem | `begin` | `i + 1 - begin` | não |
| `\u` é o fim (:248-255) | `InvalidUnicodeEscapeUStarted` → `INVALID_UNICODE_ESCAPE_U_STARTED` | `begin` | `L + 1 - begin` | não |
| `\u{` é o fim (:260-267), ou o fim chega dentro das chaves (:270-277) | `InvalidUnicodeEscapeUBracket` → `INVALID_UNICODE_ESCAPE_U_BRACKET` | `begin` | `i + 1 - begin` | não |
| não-hex dentro das chaves, inclusive `}` logo depois de `{` (:285-292) | idem | `begin` | `i + 2 - begin` | não |
| 7 caracteres lidos sem `}` (:295-301) | idem | `begin` | `i + 1 - begin` | **sim** (sem `return`) |
| `\uXXXX` com menos de 4 depois (:304-311) | `InvalidUnicodeEscapeUNoBracket` → `INVALID_UNICODE_ESCAPE_U_NO_BRACKET` | `begin` | `L + 1 - begin` | não |
| `\uXXXX` com não-hex (:315-322) | idem | `begin` | `i + 1 - begin` | não |
| valor > `0x10FFFF` (:326-330) | `InvalidCodePoint` → `INVALID_CODE_POINT`, argumento fixo `'\u{...}'` (`an:fasta/error_converter.dart:264-270`) | `begin` | `i + 1 - begin` | não |

- No primeiro erro com "não" a função devolve o texto cru: **no máximo um erro por trecho**, salvo o par
  `U_BRACKET` + `INVALID_CODE_POINT` de `\u{1234567}` (exemplo a23).
- Escapes desconhecidos (`\q`, `\$`) passam o caractere adiante sem erro (:331-333). `\uD800` é válido.
- **Offset relatado ≠ posição da `\` em geral.** `stringOffset` é índice no trecho **já sem a aspa de
  abertura** (`unescapeString`/`unescapeFirstStringPart` cortam `firstQuoteLength`, :89-107, :125-149),
  e é somado a `token.charOffset` sem repor esse corte. Com `begin` = índice do `x`/`u`:
  - string ou primeiro trecho com aspa simples (`firstQuoteLength = 1`): o offset cai **na `\`**
    (exemplos a20–a23);
  - multilinha (`firstQuoteLength` = 3 + espaços/tabs + quebra de linha, `lengthOfOptionalWhitespacePrefix`,
    :62-87): o offset cai `firstQuoteLength - 1` posições **antes** da `\` (exemplo a56: `\` em 28, erro
    em 25);
  - trecho do meio ou final de string interpolada (nenhum corte no início): o offset cai **no `x`/`u`**,
    uma posição depois da `\` (exemplo a59);
  - `INVALID_UNICODE_ESCAPE_STARTED` usa `i` (índice depois da `\`): com aspa simples cai na `\`.
- Mensagens (`front_end/messages.yaml:1699-1746`): `InvalidHexEscape` "An escape sequence starting with
  '\x' must be followed by 2 hexadecimal digits."; `InvalidUnicodeEscapeUStarted` "…'\u' must be followed
  by 4 hexadecimal digits or from 1 to 6 digits between '{' and '}'."; `…UNoBracket` "…must be followed by
  4 hexadecimal digits."; `…UBracket` "…'\u{' must be followed by 1 to 6 hexadecimal digits followed by a
  '}'."; `InvalidEscapeStarted` "The string '\' can't stand alone." (correção "Try adding another
  backslash (\) to escape the '\'."); `INVALID_CODE_POINT` (`analyzer/messages.yaml:21415`) "The escape
  sequence '{0}' isn't a valid code point.".

#### E.1.6 Comentários, `#!`, BOM, caracteres fora do ASCII

- `tokenizeSlashOrComment` (:1477-1491): `/*` → `tokenizeMultiLineComment`; `//` →
  `tokenizeSingleLineComment` (:1590-1611: até LF, CR ou EOF; `///` é dartdoc); `/=`; `/`.
- `tokenizeMultiLineComment` (:1613-1670): contador `nesting` (comentários de bloco aninham); `/**` é
  dartdoc. Em `$EOF`: `prependErrorToken(UnterminatedToken(messageUnterminatedComment, tokenStart,
  stringOffset))` e fim — **nenhum** token de comentário é criado e tudo depois do `/*` foi engolido. O
  analyzer relata `UNTERMINATED_MULTI_LINE_COMMENT` em `endOffset - 1` = **último caractere do arquivo**
  (em geral o LF final; exemplos a04, a41, b19), length 1. Código fora da lista da família (sem perda).
- `tokenizeTag` (:1002-1019): `#!` só com `scanOffset == 0` → `SCRIPT_TAG` até LF/CR/EOF (exclusive);
  em qualquer outro lugar `#` é `HASH` e o `!` segue como token (exemplo a36). Como o BOM já foi removido
  do texto, `BOM` + `#!` é script tag.
- **BOM**: não chega ao scanner (E.1.0); os offsets não o contam (exemplo a37). Um `U+FEFF` no meio do
  texto é `NonAsciiWhitespaceToken` (lista abaixo).
- **Caractere inesperado** — `unexpected(character)` (:2020-2048) com
  `buildUnexpectedCharacterToken(character, tokenStart)` (`fe:scanner/error_token.dart:25-61`):

| caractere (unidade UTF-16) | ErrorToken | recuperação |
|---|---|---|
| `< 0x1f` | `AsciiControlCharacterToken` | caractere pulado, nenhum token |
| `U+FFFD` | `EncodingErrorToken` (`messageEncoding`, **sem** `analyzerCode`) | `translateErrorToken` lança `UnimplementedError` (:90) — o analyzer **cai** (exemplo a46) |
| `U+00A0`, `U+1680`, `U+180E`, `U+2000`–`U+200B`, `U+2028`, `U+2029`, `U+202F`, `U+205F`, `U+3000`, `U+FEFF` | `NonAsciiWhitespaceToken` | caractere pulado, nenhum token |
| qualquer outro (inclusive `0x1f`, `0x7f`, metades de par substituto) | `NonAsciiIdentifierToken` | vira (parte de) um identificador |

  Recuperação do `NonAsciiIdentifierToken` (:2023-2043): se o token anterior é `IDENTIFIER` e termina
  exatamente em `tokenStart`, ele é **retirado** da fila e seu texto vira o começo do novo identificador;
  junta o caractere ilegal; consome os caracteres de identificador ASCII seguintes (`$` incluso); anexa um
  `StringToken` `IDENTIFIER` (não sintético) no offset inicial. O próximo caractere ilegal repete o
  processo e funde de novo: `piskefløde` é **um** identificador com **um** erro; não há cascata sintática
  (o nome pode dar `undefined_identifier` com o texto original, exemplos a15, a16). Palavra-chave antes do
  caractere não funde (o teste é `tail.type == IDENTIFIER`).
- Os três tipos têm `analyzerCode: ILLEGAL_CHARACTER` (`front_end/messages.yaml:123-310`) e o argumento é
  `token.character` — **um `int`**, formatado em decimal: `Illegal character '167'.`. No `StringScanner`
  o valor é a **unidade UTF-16**: um caractere fora do BMP dá **dois** erros, um por metade do par
  (`'55357'` e `'56832'` para U+1F600), em offsets consecutivos (exemplos a16, a42).

#### E.1.7 Operadores

| entrada | tokens (função, linha) | efeito lateral |
|---|---|---|
| `=` `==` `=>` | `EQ`, `EQ_EQ`, `FUNCTION` (`tokenizeEquals`, :1170-1196) | `discardOpenLt()` antes de qualquer um |
| `===` | **um** token `EQ_EQ_EQ` + `UnsupportedOperator(tail, tokenStart)` (:1182-1185) | precedência de igualdade (`fe:scanner/token.dart:1383`) |
| `!` `!=` | `BANG`, `BANG_EQ` (`tokenizeExclamation`, :1149-1168) | |
| `!==` | **um** token `BANG_EQ_EQ` + `UnsupportedOperator` (:1157-1160) | precedência de igualdade (`token.dart:1325`) |
| `<` `<=` `<<` `<<=` | `LT` (abre grupo), `LT_EQ`, `LT_LT`, `LT_LT_EQ` (:1236-1248) | |
| `>` `>=` `>>` `>>=` `>>>` `>>>=` | `GT`, `GT_EQ`, `GT_GT`, `GT_GT_EQ`, `GT_GT_GT`, `GT_GT_GT_EQ` (:1198-1234) | `>>>`/`>>>=` só com `_enableTripleShift`; sem ela `>>` + `>` / `>>` + `>=` |
| `?` `?.` `?..` `??` `??=` | `QUESTION`, `QUESTION_PERIOD`, `QUESTION_PERIOD_PERIOD`, `QUESTION_QUESTION`, `QUESTION_QUESTION_EQ` (`tokenizeQuestion`, :1047-1067) | `?..` só com `_enableNonNullable`; sem ela `?.` + `.` |
| `.` `..` `...` `...?` `.5` | `PERIOD`, `PERIOD_PERIOD`, `PERIOD_PERIOD_PERIOD`, `PERIOD_PERIOD_PERIOD_QUESTION`, DOUBLE (`tokenizeDotsOrNumber`, :1352-1376) | |
| `&` `&&` `&=` | `AMPERSAND`, `AMPERSAND_AMPERSAND`, `AMPERSAND_EQ` (:1089-1107) | `&&=` não existe (`LAZY_ASSIGNMENT_ENABLED = false`, :66): `&&` + `=` |
| `\|` `\|\|` `\|=` | `BAR`, `BAR_BAR`, `BAR_EQ` (:1069-1087) | `\|\|=` idem |
| `+` `++` `+=` / `-` `--` `-=` | (:1134-1147) / (:1119-1132) | |
| `*` `*=`, `%` `%=`, `^` `^=` | `select('=')` (:1114, :1109, :1042) | |
| `~` `~/` `~/=` | `TILDE`, `TILDE_SLASH`, `TILDE_SLASH_EQ` (:1021-1030) | |
| `/` `/=` `//` `/*` | (:1477-1491) | |
| `[` `[]` `[]=` | `OPEN_SQUARE_BRACKET` (grupo), `INDEX`, `INDEX_EQ` (:1032-1040) | |
| `#` / `#!` no offset 0 | `HASH` / `SCRIPT_TAG` (:1002-1019) | |
| `@`, `,`, `:`, `;` | `AT`, `COMMA`, `COLON`, `SEMICOLON` | `;` chama `discardOpenLt()` |
| `` ` ``, `\` | `BACKPING`, `BACKSLASH` (:979-987) | sem erro léxico |

Depois de `===`/`!==` o scanner segue no caractere seguinte: `====` é `===` + `=` (exemplo b20).

#### E.1.8 Palavras-chave e identificadores

`tokenizeKeywordOrIdentifier(next, allowDollar)` (:1723-1766): percorre a trie `KeywordState.KEYWORD_STATE`
(`fe:scanner/keyword_state.dart:21-30`, montada com todos os `Keyword.values`); o primeiro caractere pode
ser maiúsculo (`nextCapital`, para `Function`), os seguintes só `a`–`z`. Sai como `tokenizeIdentifier`
quando: a trie não tem o caminho; o estado não é palavra completa; a palavra é `extension` sem
`_enableExtensionMethods`, `late`/`required` sem `_enableNonNullable`, `augment` sem
`_forAugmentationLibrary` (:1747-1756); ou o caractere seguinte é `A`–`Z`, dígito, `_` ou (com
`allowDollar`) `$` (:1757-1761). Senão `appendKeywordToken` (:340-347; `this` chama `discardOpenLt()`).
**Todas** as palavras de `Keyword.values` (reservadas, embutidas e pseudo) saem como `KeywordToken`; o
parser decide pelo estilo (ver T4 da família E).

`tokenizeIdentifier(next, start, allowDollar)` (:1772-1788) consome `_isIdentifierChar` (:2197-2203:
`a`–`z`, `A`–`Z`, `0`–`9`, `_`, e `$` se `allowDollar`); só ASCII. `$` inicia e continua identificador
fora de string (`bigSwitch`, :955-957); dentro de `"$nome"` o nome é lido com `allowDollar: false`, então
`"$a$b"` são duas interpolações e `"$$a"` é um `$` sem nome (erro) seguido de `$a` (exemplo a25).

#### E.1.9 `translateErrorToken` — tabela completa

`fe:scanner/errors.dart:15-93`. `charOffset = token.charOffset`; `endOffset = token.endOffset ??
charOffset`. O despacho é por `errorCode.analyzerCodes?.first` (o `analyzerCode` da mensagem fasta em
`front_end/messages.yaml`). Length sempre 1 (`reportScannerError`).

| ErrorToken (onde nasce) | mensagem fasta / `analyzerCode` | `ScannerErrorCode` | offset | argumentos |
|---|---|---|---|---|
| `UnterminatedString` (`unterminatedString`, :2066) | `UnterminatedString` / `UNTERMINATED_STRING_LITERAL` | `UNTERMINATED_STRING_LITERAL` | `endOffset - 1`, sem `_makeError` (:32-37) | — |
| `UnterminatedToken(messageUnterminatedComment)` (:1623) | `UnterminatedComment` / `UNTERMINATED_MULTI_LINE_COMMENT` | `UNTERMINATED_MULTI_LINE_COMMENT` | `endOffset - 1`, sem `_makeError` (:39-44) | — |
| `UnterminatedToken(messageMissingExponent)` (:1430) | `MissingExponent` / `MISSING_DIGIT` | `MISSING_DIGIT` | `endOffset - 1` (:46-50) | — |
| `UnterminatedToken(messageExpectedHexDigit)` (:1331) | `ExpectedHexDigit` / `MISSING_HEX_DIGIT` | `MISSING_HEX_DIGIT` | `endOffset - 1` (:52-56) | — |
| `AsciiControlCharacterToken`, `NonAsciiWhitespaceToken`, `NonAsciiIdentifierToken` (`unexpected`, :2020) | `AsciiControlCharacter`, `NonAsciiWhitespace`, `NonAsciiIdentifier` / `ILLEGAL_CHARACTER` | `ILLEGAL_CHARACTER` | `charOffset` (:58-61) | `[token.character]` (int) |
| `UnterminatedToken(messageUnexpectedSeparatorInNumber)` (:1266 e outros) | `UnexpectedSeparatorInNumber` / `UNEXPECTED_SEPARATOR_IN_NUMBER` | `UNEXPECTED_SEPARATOR_IN_NUMBER` | `charOffset` = início do número (:63-64) | — |
| `UnsupportedOperator` (:1159, :1184) | `UnsupportedOperator` / `UNSUPPORTED_OPERATOR` | `UNSUPPORTED_OPERATOR` | `charOffset` = início do operador (:66-68) | `[token.token.lexeme]` (`===` ou `!==`) |
| `UnmatchedToken` (`unmatchedBeginGroup`, :745) | `UnmatchedToken` / `EXPECTED_TOKEN` (ramo `default`, por identidade `codeUnmatchedToken`, :71-86) | `EXPECTED_TOKEN` | `begin.endToken.charOffset` (fecho sintético, talvez movido) | `'}'` (abridor `{` ou `${`), `']'`, `')'`, `'>'` |
| `UnterminatedToken(messageUnexpectedDollarInString)` (:1895) | `UnexpectedDollarInString` / `UNEXPECTED_DOLLAR_IN_STRING` (ramo `default`, :87-88) | **`MISSING_IDENTIFIER`** | `charOffset` = caractere depois do `$` | — |
| `EncodingErrorToken` (`U+FFFD`) | `Encoding` / sem `analyzerCode` | — | — | lança `UnimplementedError` (:90-91) |

Os ramos com `_makeError` (:19-28) aplicam `_isAtEnd` (E.1.2). `ScannerErrorCode.MISSING_QUOTE`,
`UNABLE_GET_CONTENT` e `UNEXPECTED_DOLLAR_IN_STRING` (`fe:scanner/errors.dart:148-163`) existem como
constantes mas `translateErrorToken` não os produz. Mensagens: `fe:scanner/errors.dart:129-188`.

### E.1 Scanner — algoritmo e recuperação (rodada 4)

Fonte: `E:\references\dart-sdk-3.6.2\pkg`, citada como `fe:` = `_fe_analyzer_shared/lib/src/` e
`an:` = `analyzer/lib/src/` (linhas do 3.6.2 conferidas nesta rodada). Exemplos rodados no oráculo
vivo (`C:\tools\dartsdk-3.6.2`), arquivos em `E:\dftemp\analise\spec-r4\casos\e1\` (`gen.py` gera e roda;
`saida.txt` tem a saída bruta; o nome `aNN`/`bNN`/`cNN` de cada exemplo é o do arquivo). Perdas de
`familia-E.txt` (placar `r7`). Esta parte aprofunda e corrige o T3 e o §SCANNER da família E (rodada 3).

#### E.1.0 Onde o scanner entra e por onde os erros saem

- **Entrada.** `FileState.parseCode` (`an:dart/analysis/file_state.dart:655-690`) cria
  `Scanner(source, CharSequenceReader(code), errorListener)` e chama
  `scanner.tokenize(reportScannerErrors: false)` (:671). O `Scanner` do analyzer
  (`an:dart/scanner/scanner.dart:125-164`) chama `fasta.scanString(_contents, configuration:
  buildConfig(_featureSet), includeComments: true, languageVersionChanged: …)` — é o **`StringScanner`**
  (`fe:scanner/string_scanner.dart:38`), não o `Utf8BytesScanner`: `advance()` devolve **unidades
  UTF-16** (:74-79), `stringOffset == scanOffset` (:90), `currentAsUnicode(next) => next` (:93). Todo
  offset do scanner é em unidades UTF-16 do texto já decodificado.
- **Texto.** `an:dart/analysis/file_content_cache.dart:49-50`: `utf8.decode(readAsBytesSync())`. O
  decodificador remove o BOM inicial (comportamento de `dart:convert`, não conferido no fonte do SDK;
  conferido no oráculo: exemplo a37, offsets iguais aos do arquivo sem BOM). UTF-8 inválido lança, o
  `catch` (:52-56) dá conteúdo vazio e `exists = false`.
- **Configuração** (`an:dart/scanner/scanner.dart:198-207`): `enableExtensionMethods`,
  `enableTripleShift` (`Feature.triple_shift`), `enableNonNullable`, `forAugmentationLibrary`
  (= `Feature.macros`). O comentário `// @dart=x.y` do cabeçalho chama `_languageVersionChanged`
  (:166-195), que troca a configuração **no meio da varredura** (`scanner.configuration = …`).
- **Erros.** O scanner nunca para e nunca relata: cada erro vira um `ErrorToken`
  (`fe:scanner/error_token.dart:67`, `TokenType.BAD_INPUT`) posto **no início da fila** por
  `prependErrorToken` (`fe:scanner/abstract_scanner.dart:498-510`: `errorTail` aponta o último
  ErrorToken; os ErrorToken ficam contíguos na cabeça, na ordem de detecção), e o token "bom" é
  sintetizado no lugar. `_tokenizeAndRecover` (`fe:scanner/scanner.dart:96-104`) só confere, com
  `scannerRecovery` (`fe:scanner/recover.dart:17-49`), que não há ErrorToken fora da cabeça (senão lança
  "Internal error: All error tokens should have been prepended").
- **Relato.** `Parser.parseUnit` (`fe:parser/parser_impl.dart:403-444`): `skipErrorTokens(errorToken)`
  (:407, definição :9461) pula a cabeça; a unidade inteira é analisada; **no fim**
  `reportAllErrorTokens(errorToken)` (:440, definição :9453) chama `listener.handleErrorToken` para cada
  um ("so that the parser has the chance to adjust the error location", :404-405).
  `AstBuilder.handleErrorToken` (`an:fasta/ast_builder.dart:4206-4208`) →
  `translateErrorToken(token, errorReporter.reportScannerError)`; `FastaErrorReporter.reportScannerError`
  (`an:fasta/error_converter.dart:578-588`) relata com **`length = 1` fixo**.
  Consequências: (1) na lista da unidade, todos os erros do scanner vêm **depois** de todos os do
  parser/AstBuilder (o `dart analyze` reordena por severidade e offset; a ordem de emissão não é
  visível no JSON); (2) o comprimento é sempre 1, mesmo além do fim do arquivo; (3) o parser pode ter
  **movido** o fecho sintético antes do relato (E.1.2).
- O caminho `reportScannerErrors: true` (`an:dart/scanner/scanner.dart:145-152`) só é usado por
  `an:fasta/doc_comment_builder.dart:422` (código dentro de comentário de documentação).
- **Duplicatas.** Dois erros idênticos (mesmo `ErrorCode`, offset, length, mensagem) saem uma vez só
  (exemplo c02: `0x__1` cria dois ErrorToken iguais e o oráculo mostra um; onde a duplicata some: não
  verificado). `ParserErrorCode.EXPECTED_TOKEN` e `ScannerErrorCode.EXPECTED_TOKEN` são códigos
  **diferentes** com o mesmo nome: o par parser+scanner no mesmo lugar sai **duas vezes** (exemplo c01).

#### E.1.1 Laço principal

`AbstractScanner.tokenize` (`fe:scanner/abstract_scanner.dart:753-785`):

```
tokenize():
  enquanto !atEndOfFile():
    next = advance()
    se next != EOF:                       # cabeçalho: até o 1º token real
      oldTail = tail
      next = bigHeaderSwitch(next)
      se next != EOF e tail.kind == SCRIPT_TOKEN:    # depois do `#!`, o cabeçalho continua
        oldTail = tail; next = bigHeaderSwitch(next)
      enquanto next != EOF e tail == oldTail:        # comentários e espaço não mudam `tail`
        next = bigHeaderSwitch(next)
    enquanto next != EOF: next = bigSwitch(next)
    se atEndOfFile(): appendEofToken()  senão: unexpectedEof()
  lineStarts.add(stringOffset + 1)        # linha fictícia no fim; o analyzer a descarta (scanner.dart:131-138)
  devolve firstToken()
```

- `$EOF = -1` e `$STX = 2` (`fe:scanner/characters.dart:7-8`). No `StringScanner`, `advance()` só devolve
  `$EOF` além do fim (:77), então `unexpectedEof` (:2050) não é alcançado; um caractere NUL no texto é
  caractere de controle comum (exemplo b11).
- `bigHeaderSwitch` (:816-825): `/` seguido de `/` → `tokenizeLanguageVersionOrSingleLineComment` (:1493:
  `//` + espaços + `@dart` + espaços + `=` + espaços + dígitos `.` dígitos + espaços + fim de linha; `///`
  não vale, :1498; tabulação não vale); qualquer outra coisa → `bigSwitch`. O marcador só é reconhecido
  **antes do primeiro token** (o laço `tail == oldTail`).
- `bigSwitch(next)` (:827-1000): `beginToken()` (`tokenStart = stringOffset`, :256) e despacho pela ordem
  do fonte:

| caractere | função (linha) | observação |
|---|---|---|
| espaço, TAB, LF, CR | `appendWhiteSpace` (:829-839) | LF acrescenta `lineStarts`; só esses quatro (FF `0x0C` é ilegal) |
| `a`–`z`, `A`–`Z` | `r` → `tokenizeRawStringKeywordOrIdentifier` (:845); senão `tokenizeKeywordOrIdentifier(next, allowDollar: true)` (:847) | teste por `next \| 0x20` |
| `)` | `appendEndGroup(CLOSE_PAREN, OPEN_PAREN_TOKEN)` (:851) | E.1.2 |
| `(` | `appendBeginGroup(OPEN_PAREN)` (:855) | não descarta `<` |
| `;` | `SEMICOLON` + `discardOpenLt()` (:859-864) | |
| `.` | `tokenizeDotsOrNumber` (:867) | `.5`, `..`, `...`, `...?`, `.` |
| `,` | `COMMA` (:871) | |
| `=` | `tokenizeEquals` (:876) | `discardOpenLt()` antes |
| `}` | `appendEndGroup(CLOSE_CURLY_BRACKET, OPEN_CURLY_BRACKET_TOKEN)` (:880) | também fecha `${` |
| `/` | `tokenizeSlashOrComment` (:885) | `/*`, `//`, `/=`, `/` |
| `{` | `appendBeginGroup(OPEN_CURLY_BRACKET)` (:889) | descarta `<` |
| `"` `'` | `tokenizeString(next, scanOffset, raw: false)` (:894) | |
| `_` | `tokenizeKeywordOrIdentifier` (:898) | |
| `:` | `COLON` (:902) | |
| `<` | `tokenizeLessThan` (:907) | `<=`, `<<`, `<<=`; `<` só abre grupo |
| `>` | `tokenizeGreaterThan` (:911) | `>=`, `>>`, `>>=`, `>>>`, `>>>=` |
| `!` | `tokenizeExclamation` (:915) | `!=`, `!==` (erro) |
| `[` | `tokenizeOpenSquareBracket` (:919) | `[]` e `[]=` são UM token, sem grupo |
| `]` | `appendEndGroup(CLOSE_SQUARE_BRACKET, OPEN_SQUARE_BRACKET_TOKEN)` (:923) | |
| `@` | `tokenizeAt` (:928) | |
| `1`–`9` | `tokenizeNumber` (:932) | |
| `&` | `tokenizeAmpersand` (:936) | |
| `0` | `tokenizeHexOrNumber` (:940) | |
| `?` | `tokenizeQuestion` (:944) | |
| `\|` | `tokenizeBar` (:948) | |
| `+` `-` `*` `^` `~` `%` | `tokenizePlus` (:952), `tokenizeMinus` (:960), `tokenizeMultiply` (:964), `tokenizeCaret` (:968), `tokenizeTilde` (:972), `tokenizePercent` (:976) | |
| `$` | `tokenizeKeywordOrIdentifier(next, allowDollar: true)` (:956) | `$` inicia identificador |
| `` ` `` | `BACKPING` (:980) | token válido para o scanner; o parser recusa (exemplo a54) |
| `\` | `BACKSLASH` (:985) | idem |
| `#` | `tokenizeTag` (:990) | `#!` só em `scanOffset == 0` |
| `< 0x1f` (resto) | `unexpected(next)` (:993-995) | `AsciiControlCharacterToken` |
| qualquer outro | `unexpected(currentAsUnicode(next))` (:997-999) | `0x1f`, `0x7f` e toda unidade ≥ `0x80` |

Toda a pontuação ASCII imprimível tem ramo próprio; só chegam a `unexpected` os controles (fora TAB, LF,
CR), `0x1f`, `0x7f` e as unidades não ASCII (E.1.6).

#### E.1.2 Grupos e fechos

Estado: `groupingStack` (`Link<BeginToken>`, :141), topo = grupo mais interno. Abrem grupo: `(`, `[`,
`{`, `<` solto e `${`. Cada `BeginToken` (`fe:scanner/token.dart:38`) recebe `endGroup` quando o fecho é
lido ou sintetizado.

**Abertura** — `appendBeginGroup(type)` (:386-395): cria o `BeginToken` em `tokenStart`, anexa; se o tipo
não é `<` nem `(`, chama `discardOpenLt()` (`{`, `[` e `${` não cabem em argumentos de tipo); empilha.
`[]` e `[]=` (`tokenizeOpenSquareBracket`, :1032-1040) não abrem grupo: são os tokens `INDEX`/`INDEX_EQ`
(o parser os reparte com `rewriteSquareBrackets`, `fe:parser/parser_impl.dart:4317`).

**`<` e `>`** — `tokenizeLessThan` (:1236-1248): `<=` → `LT_EQ`; `<<`/`<<=` → `LT_LT`/`LT_LT_EQ`; `<`
sozinho → `appendBeginGroup(LT)`. `tokenizeGreaterThan` (:1198-1234): `>=`, `>>=`, `>>>=` são operadores
sem efeito na pilha; `>` → `appendGt` (:444-451: se o topo é `<`, `endGroup = tail` e desempilha); `>>`
→ `appendGtGt` (:458-471: desempilha o `<` do topo **sem** `endGroup`, e o seguinte, se for `<`, recebe
`endGroup`); `>>>` → `appendGtGtGt` (:477-495: dois sem `endGroup`, o terceiro com), só com
`_enableTripleShift` (:1212); sem a flag, `>>>` é `>>` seguido de `>`. Nenhum `>` relata erro ("it does
not necessarily have to close a group").

**`discardOpenLt()`** (:676-680): desempilha todos os `<` do topo, sem erro e sem `endGroup`. Chamado
por: `;` (:862), todo token que começa com `=` (:1176), a palavra-chave `this` (:344), `{`/`[`/`${`
(:392), o fim do arquivo (:351) e cada passo de `discardBeginGroupUntil` (:556).

**Fecho** — `appendEndGroup(type, openKind)` (:403-407) = `discardBeginGroupUntil(openKind)` +
`appendEndGroupInternal` (:414-437):

```
discardBeginGroupUntil(openKind):                     # :550-650
  originalStack = groupingStack; first = true
  faça:
    discardOpenLt()
    se groupingStack vazia: pare
    begin = topo
    se begin.kind == openKind  ou  (openKind == '{' e begin.kind == '${'):
      se first: devolve true                          # casou de primeira: sem recuperação
      pare
    first = false; groupingStack = groupingStack.tail
  enquanto groupingStack não vazia
  recoveryCount++
  se groupingStack vazia:                             # não há abridor desse tipo na pilha
    groupingStack = originalStack; devolve false      # o fecho é ignorado: token comum, pilha intacta
  se !inRecoveryOption:                               # :592-644, só no scanner principal
    opção 1 = cópia do scanner (createRecoveryOptionScanner): insertSyntheticClosers + fecha o grupo;
              custo1 = recoveryCount depois de até 100 bigSwitch + tamanho da pilha que sobrou
    opção 2 = cópia com a pilha original, fecho ignorado;
              custo2 = idem + 1
    zera endToken dos abridores de originalStack      # :632-636
    se custo2 < custo1: groupingStack = originalStack; devolve false
  insertSyntheticClosers(originalStack, groupingStack); devolve true

appendEndGroupInternal(found, type, openKind):
  se !found: appendPrecedenceToken(type); devolve advance()      # fecho solto: token simples
  appendPrecedenceToken(type); begin = topo; begin.endGroup = tail; desempilha
  se begin.kind != openKind: devolve $STX             # era `${`: volta ao modo string
  devolve advance()
```

- `recoveryOptionTokenizer` (:790-814) roda `bigSwitch` no máximo 100 vezes; as cópias têm
  `inRecoveryOption = true` (:163), então dentro delas a escolha é sempre a opção 1 (sem recursão
  exponencial). Empate → opção 1 (inserir fechos). Exemplos do fonte (:589-591): `{[}` → `{[]}`;
  `[(])]` → o primeiro `]` é ignorado (opção 2; exemplo a44: os dois diagnósticos são do **parser**).
- `insertSyntheticClosers(originalStack, entryToUse)` (:652-663): do topo da pilha original até o
  abridor casado (exclusive), `unmatchedBeginGroup(abridor)` — **o mais interno primeiro**. A guarda
  `entryToUse.head.kind != LT_TOKEN` olha o abridor **casado**, não o descartado: um `<` que ficou
  **abaixo** de um `(` na pilha é relatado (exemplo b01, `Expected to find '>'`). Um `<` no topo nunca é
  relatado (o `discardOpenLt` o tira antes).
- `unmatchedBeginGroup(begin)` (:697-747): anexa `SyntheticToken(closeBraceInfoFor(begin), tokenStart)`
  com `beforeSynthetic = tail` (o fecho sintético tem **comprimento 0** e offset = início do token que
  estava sendo lido: o fecho que não casou, ou o fim do arquivo); `begin.endGroup = tail`;
  `prependErrorToken(UnmatchedToken(begin))` (`fe:scanner/error_token.dart:223-237`, `charOffset =
  begin.charOffset`); `recoveryCount++`. `closeBraceInfoFor` (:2075-2083): `(`→`)`, `[`→`]`, `{`→`}`,
  `<`→`>`, `${`→`}`.
- **Fim do arquivo** — `appendEofToken` (:349-357): `beginToken()`, `discardOpenLt()`, depois
  `unmatchedBeginGroup` para cada grupo restante, do topo para a base, e o `EOF` em `tokenStart`. Os
  fechos sintéticos ficam todos no offset do EOF (= comprimento do texto).
- **`${` sem fecho** — `tokenizeInterpolatedExpression` (:1868-1883): roda `bigSwitch` até `$EOF` ou
  `$STX`; em `$EOF`: `beginToken()` + `discardInterpolation()` (:688-695: `unmatchedBeginGroup` de cada
  grupo do topo até o `${`, **inclusive**) e a string externa cai em `unterminatedString` (E.1.4). A
  interpolação **atravessa quebras de linha**: `"x${a;` numa linha engole as linhas seguintes até achar
  um `}` que case ou o fim (exemplos a02, a45).
- **Fecho a mais** (`}` ou `)` sem abridor): opção "pilha vazia" → token simples; quem relata é o parser
  (exemplos a29, a30).

**Onde sai o erro** — `translateErrorToken`, ramo `codeUnmatchedToken` (`fe:scanner/errors.dart:71-86`):
`charOffset = token.begin!.endToken!.charOffset` = offset **atual** do fecho sintético;
`ScannerErrorCode.EXPECTED_TOKEN` com `'}'` (abridor `{` ou `${`), `']'`, `')'` ou `'>'`; length 1.
O parser move o fecho sintético com `TokenStreamRewriter.moveSynthetic`
(`fe:parser/token_stream_rewriter.dart:80-110`: tira o fecho e o `UnmatchedToken` vizinho de onde estão,
reinsere depois de `token` e faz `_setOffset(endGroup, next.offset)`), então o erro do scanner sai no
**token que segue o ponto onde o parser esperava o fecho**. Chamadores de `moveSynthetic` no 3.6.2
(`fe:parser/parser_impl.dart`): `ensureCloseParen` (:4234-4253, move em :4242 **sem** relatar nada: só o
erro do scanner sai), `parseArgumentOrIndexStar` (:6492, relata `ExpectedButGot(']')` **e** move → dois
`expected_token` iguais, exemplo c01), `parseLiteralListSuffix` (:6964), `parseFormalParametersRest`
(:1818), `parseRecordType` (:1652), `parseEnum` (:2400), `parseConditionalUri` (:1090),
`parseTryStatement` (:8899, :8920), `parseAssert` (:9169), `parseListPatternSuffix` (:10012). Onde
ninguém move (blocos `{`, literais de mapa/conjunto, records), o erro fica onde o scanner pôs o fecho
(exemplos a11, b04, c07). `ensureCloseParen` com `)` **real** mais adiante (:4245-4252) relata
`ParserErrorCode.EXPECTED_TOKEN` no token corrente e salta para o `endGroup`.

**`_isAtEnd`** (`fe:scanner/errors.dart:99-111`, usado por `_makeError` :19-28): recua 1 o offset se
entre o ErrorToken e o EOF só houver ErrorTokens e o offset for o do EOF. Como os ErrorToken estão na
cabeça da fila, basta existir um token real para o teste dar falso: nos exemplos rodados nunca recuou
(fecho que falta no EOF sai em `offset = comprimento do arquivo`, **além do fim**, com length 1:
exemplos a11, a31, c04; `'$` no EOF, exemplo b18).

**Ordem com vários fechos faltando.** Emissão: mais interno primeiro (`(`, depois `[`, depois `{`), todos
no fim da lista da unidade (depois dos erros do parser). No mesmo offset, o JSON do `dart analyze` os
mostra em ordem `)`, `]`, `}` nos casos rodados (a31, a50); em a50 a emissão é `]` e depois `)`, e o
JSON mostra `)` antes — a ordem do JSON não é a de emissão (critério de desempate do `dart analyze`:
não verificado).

#### E.1.3 Números

`tokenizeHexOrNumber` (:1300-1306): `0` seguido de `x`/`X` → `tokenizeHex`; senão `tokenizeNumber`.

```
tokenizeNumber(next):                                    # :1250-1298
  start = scanOffset; hasSeparators = previousWasSeparator = false
  laço: next = advance()
    dígito → previousWasSeparator = false
    '_'    → hasSeparators = previousWasSeparator = true
    'e'/'E'→ se previousWasSeparator: erro SEP;  devolve tokenizeFractionPart(next, start, hasSeparators)
    '.'    → se previousWasSeparator: erro SEP
             se peek() é dígito: devolve tokenizeFractionPart(peek, start, hasSeparators)
             senão: token INT (ou INT_WITH_SEPARATORS); devolve '.'      # `1.` = INT e depois PERIOD
    outro  → se previousWasSeparator: erro SEP;  token INT/INT_WITH_SEPARATORS; devolve next

tokenizeHex(next):                                       # :1308-1350
  start = scanOffset; advance()  # passa o x/X
  laço: next = advance()
    dígito hex → hasDigits = true; previousWasSeparator = false
    '_'        → se !hasDigits: erro SEP;  hasSeparators = previousWasSeparator = true
    outro      → se !hasDigits:
                   prepend UnterminatedToken(messageExpectedHexDigit, start, stringOffset)
                   token sintético HEXADECIMAL = texto lido + "0"  (appendSyntheticSubstringToken)
                   devolve next
                 se previousWasSeparator: erro SEP
                 token HEXADECIMAL (ou HEXADECIMAL_WITH_SEPARATORS); devolve next

tokenizeFractionPart(next, start, hasSeparators):        # :1378-1475
  hasDigit = previousWasSeparator = false
  laço:
    dígito → hasDigit = true
    '_'    → se !hasDigit: erro SEP;  hasSeparators = previousWasSeparator = true
    'e'/'E'→ se previousWasSeparator: erro SEP
             hasDigit = true; next = advance()
             enquanto next == '_': erro SEP; next = advance()
             se next é '+' ou '-': next = advance()
             laço do expoente:
               dígito → hasExponentDigits = true
               '_'    → se !hasExponentDigits: erro SEP;  hasSeparators = previousWasSeparator = true
               outro  → se !hasExponentDigits:
                          token sintético DOUBLE = texto lido + "0"
                          prepend UnterminatedToken(messageMissingExponent, tokenStart, stringOffset)
                          devolve next
                        sai
             se previousWasSeparator: erro SEP;  fim
    outro  → se previousWasSeparator: erro SEP;  fim
  token DOUBLE (ou DOUBLE_WITH_SEPARATORS); devolve next

erro SEP = prepend UnterminatedToken(messageUnexpectedSeparatorInNumber, start, stringOffset)
```

- `tokenizeDotsOrNumber` (:1352-1376): `.` + dígito → `tokenizeFractionPart` (o `.5` é DOUBLE); `..`,
  `...`, `...?`; senão `PERIOD`. O ramo `!hasDigit` de `tokenizeFractionPart` (:1457-1470) não é
  alcançado por nenhum dos três chamadores (todos entram com dígito ou `e`) — leitura do fonte.
- Limites de token: `1.e` = `INT(1)` `.` `e` (exemplo a08, sem erro léxico); `1e`, `1e+`, `1.5e`, `1.5e-`
  = um DOUBLE sintético (lexema com `0` acrescentado, `length` = só o texto real:
  `SyntheticStringToken`, `fe:scanner/string_scanner.dart:109-117`); `0x`, `0X` = HEXADECIMAL sintético
  `0x0`; `0xg` = HEXADECIMAL sintético + identificador `g` (exemplo a51).
- **Separadores `_`: existem no 3.6.2.** O scanner os aceita sempre (não há flag no
  `ScannerConfiguration`, :2169-2195); o tipo do token muda para `*_WITH_SEPARATORS`
  (`fe:scanner/token.dart:1267`), o parser chama `parseLiteralIntWithSeparators` /
  `parseLiteralDoubleWithSeparators` (`fe:parser/parser_impl.dart:6565-6580`) e o AstBuilder
  (`an:fasta/ast_builder.dart:4729-4744`, :4767-4784) relata `experiment_not_enabled`
  (`digit-separators`, versão `3.6.0`: `an:dart/analysis/experiments.g.dart:236-243`) no token inteiro se
  a biblioteca é anterior à 3.6 (exemplo b09) e calcula o valor com `stripSeparators`. Separador fora de
  lugar → `ScannerErrorCode.UNEXPECTED_SEPARATOR_IN_NUMBER` (fora da lista da família; o DartForge já o
  relata, E.1.12). Regras (do pseudocódigo): proibido antes de `e`/`E`, antes de `.`, no fim, antes do
  primeiro dígito hex, logo depois de `e` ou do sinal do expoente. `1__0` é válido. `1._5` é
  `INT` `.` `_5` (sem erro léxico; exemplo b10).
- **Posições** (`fe:scanner/errors.dart:46-64`): `MISSING_DIGIT` e `MISSING_HEX_DIGIT` em `endOffset - 1`,
  onde `endOffset = stringOffset` no momento da detecção = offset do primeiro caractere que não pertence
  ao número → o erro cai no **último caractere do número** (`e`, o sinal, `x`, ou o `_` de `0x_`).
  `UNEXPECTED_SEPARATOR_IN_NUMBER` em `token.charOffset` = `start` = **primeiro caractere do número**.
  Length 1 em todos.
- Valor: `handleLiteralInt` (`an:fasta/ast_builder.dart:4753-4764`) usa `int.tryParse(token.lexeme)`; o
  lexema sintético `0x0` vale 0 e `1e0` vale 1.0. `0x_` vira o lexema `0x_0`, que `int.tryParse` recusa →
  também sai `integer_literal_out_of_range` (exemplo b10; código fora da família).


#### E.1.11 Exemplos do oráculo vivo (3.6.2)

Todos rodados com `dart analyze --format=json` (um arquivo por vez). Formato de cada linha:
`código` offset/length (linha:coluna) mensagem. A ordem é a do JSON (por severidade e offset), **não** a
de emissão. "sem LF final" = o arquivo termina no último caractere mostrado.

**a01 — string sem fecho no meio de função**
```dart
void f() {
  var s = 'abc;
  print(s);
}
```
- `expected_token` 21/5 (2:11) Expected to find ';'. — parser, no token da string (`'abc;`)
- `unterminated_string_literal` 25/1 (2:15) Unterminated string literal. — no `;`, último caractere da linha

**a55 — o mesmo com CRLF**: `expected_token` 22/5 (2:11); `unterminated_string_literal` 26/1 (2:15) — o
caractere antes do CR; mais `unused_local_variable` 18/1.

**a35 — string sem fecho no EOF** (`var a = 'abc`, sem LF final): `expected_token` 8/4 (1:9) `;`;
`unterminated_string_literal` 11/1 (1:12).

**a49 — quebra de linha dentro de string simples**
```dart
var a = 'ab
cd';
```
- `expected_token` 8/3 (1:9) `;` · `unterminated_string_literal` 10/1 (1:11)
- `expected_token` 12/2 (2:1) `;` · `missing_const_final_var_or_type` 12/2 (2:1) — `cd` vira declaração
- `expected_executable` 14/2 (2:3) · `unterminated_string_literal` 15/1 (2:4) — a segunda string `';`

**a26 — multilinha sem fecho** (`var a = '''abc` + LF + `var b = 1;` + LF): `expected_token` 8/18 (1:9)
`;` (o token cobre até o fim do arquivo); `unterminated_string_literal` 25/1 (2:11) — o LF final.

**a27 — crua sem fecho** (`var a = r'abc` + LF + `var b = 1;`): `expected_token` 8/5 (1:9) `;`;
`unterminated_string_literal` 12/1 (1:13). A linha 2 é lida normalmente.

**b07 — justaposição com a segunda string aberta** (`var a = 'x' 'y` + LF + `;`): só
`unterminated_string_literal` 13/1 (1:14) (o `;` da linha 2 fecha a declaração).

**b17 — offsets em UTF-16** (`var a = 'éé;` + LF + `var b = '😀` + LF): `expected_token` 8/4 e
`unterminated_string_literal` 11/1 (1:12); `expected_token` 21/3 e `unterminated_string_literal` 23/1
(2:11) — a metade baixa do par substituto.

**a24 — `\` + quebra de linha dentro de `print(`**
```dart
void f() {
  print('abc\
');
}
```
- `invalid_unicode_escape_started` 23/1 (2:13) The string '\' can't stand alone.
- `unterminated_string_literal` 23/1 (2:13)
- `expected_token` 25/3 (3:1) Expected to find ';'. — no token `');` (segunda string, justaposta)
- `unterminated_string_literal` 27/1 (3:3)
- `expected_token` 29/1 (4:1) Expected to find ')'. — scanner: `)` sintético antes do `}`

**a40 / a58 — `\` no EOF** (`var a = 'abc\` sem LF final): `expected_token` 8/5 `;`;
`invalid_unicode_escape_started` 12/1 (1:13); `unterminated_string_literal` 12/1 (1:13). Com `var a = '\`:
os dois em 9/1.

**a02 — `${` sem fecho atravessa linhas**
```dart
void f(a) {
  var s = "x${a;
}
```
- `expected_token` 27/1 (2:16) Expected to find '}'. — parser (`parseSingleLiteralString`), no `;`
- `unterminated_string_literal` 29/1 (3:1) — o `}` da linha 3 fechou a interpolação; a string para no LF
- `expected_token` 31/0 (4:1) Expected to find ';'. — parser, length 0 no EOF
- `expected_token` 31/1 (4:1) Expected to find '}'. — scanner: `{` da função, no EOF (além do fim)
- `unused_local_variable` 18/1 (2:7)

**a03 — `"${a` no EOF** (`var s = "${a`, sem LF final): `undefined_identifier` 11/1;
`unterminated_string_literal` 11/1 (1:12); `expected_token` 12/0 (1:13) `;`; `expected_token` 12/1 (1:13)
`}` (scanner, `discardInterpolation`).

**a45 — string aberta dentro de `${`**
```dart
var s = "${a";
var t = 1;
```
- `undefined_identifier` 11/1 · `expected_token` 12/2 (1:13) `}` — parser, no token `";`
- `unterminated_string_literal` 13/1 (1:14) — a string interna `";`
- `unterminated_string_literal` 25/1 (2:11) — a externa, no LF final (a linha 2 foi lida dentro de `${`)
- `expected_token` 26/0 (3:1) `;` · `expected_token` 26/1 (3:1) `}` (scanner)

**c08 — `'${` no EOF** (`var x = '${`, sem LF final): `unterminated_string_literal` 10/1 (1:11);
`missing_identifier` 11/0 (1:12); `expected_token` 11/0 `;`; `expected_token` 11/1 `}`.

**a25 — `$` sem nome**
```dart
var a = "$";
var b = "$1";
var c = "$$a";
```
- `missing_identifier` 10/1 (1:11) · 23/1 (2:11) · 37/1 (3:11) Expected an identifier. — sempre o
  caractere depois do `$`; nenhum outro erro

**b18 — `'$` no EOF** (linha 2 `var b = '$`, sem LF final): `unterminated_string_literal` 22/1 (2:10);
`missing_identifier` 23/1 (2:11) — offset = comprimento do arquivo; `expected_token` 23/0 `;`.

**a39 — palavra-chave depois de `$`** (`var a = "$this $class";`): `invalid_reference_to_this` 10/4;
`expected_identifier_but_got_keyword` 16/5 (1:17); `undefined_identifier` 16/5.

**a20–a23 — escapes inválidos (aspa simples, erro na `\`)**
```dart
var a = '\u';          // a20: invalid_unicode_escape_u_started 9/2 (1:10)
var a = '\x1';         // a21: invalid_hex_escape 9/3 (1:10)
var a = '\u{110000}';  // a22: invalid_code_point 9/9 (1:10) The escape sequence '\u{...}' isn't a valid code point.
```
a23 (um arquivo, cinco linhas): `'\u{}'` → `invalid_unicode_escape_u_bracket` 9/4; `'\u12'` →
`invalid_unicode_escape_u_no_bracket` 25/4; `'\u{1234567}'` → `invalid_unicode_escape_u_bracket` 41/9
**e** `invalid_code_point` 41/9; `'\xZZ'` → `invalid_hex_escape` 64/2; `'\u{12'` →
`invalid_unicode_escape_u_bracket` 80/5. Todos na coluna 10 (a `\`).

**a56 — multilinha: o offset não é o da `\`**
```dart
var a = 'é\x1';
var b = '''
\u{110000}''';
```
- `invalid_hex_escape` 10/3 (1:11) — na `\`
- `invalid_code_point` 25/9 (2:10) — a `\` está em 28 (3:1); o erro cai 3 antes (`firstQuoteLength - 1`)

**a59 — trecho depois de interpolação: erro uma posição depois da `\`**
```dart
var a = "x $a \u";
var b = '${a}\x1 \u{}';
```
- `invalid_unicode_escape_u_started` 15/2 (1:16) — o `u` (a `\` está em 14)
- `invalid_hex_escape` 33/3 (2:15) — o `x` (a `\` está em 32); o `\u{}` seguinte não é relatado (um erro
  por trecho); mais `top_level_cycle` 4/1.

**a60 — sem erro**: `r'\u \x1'` (crua) e `'\q \$'` (escapes desconhecidos) não dão diagnóstico.

**a04 / a41 / b19 — comentário de bloco sem fecho**: `void f() {}` + LF + `/* abc` + LF →
`unterminated_multi_line_comment` 18/1 (2:7) (o LF final; arquivo de 19 unidades). `/* a /* b */ c` + LF +
`var a = 1;` + LF → 25/1 (2:11): o aninhamento engole o resto. `/**` + LF + ` * doc` + LF → 10/1 (2:7).

**a05, a06, a07, a47, a08 — números**
```dart
var a = 0x;     // a05: missing_hex_digit 9/1 (1:10) Hexadecimal digit expected.   (o x)
var a = 1e;     // a06: missing_digit 9/1 (1:10) Decimal digit expected.           (o e)
var a = 1e+;    // a07: missing_digit 10/1 (1:11)                                  (o +)
var a = 1.e;    // a08: só undefined_getter 10/1 "The getter 'e' isn't defined for the type 'int'."
```
a47: `var a = 1.5e;` → `missing_digit` 11/1 (1:12); `var b = .e;` → `missing_identifier` 22/1 (2:9)
(parser); `var c = 1.5e-;` → `missing_digit` 38/1 (3:13).

**a33 / a34 — no EOF** (`var a = 0x` e `var a = 1e`, sem LF final): `expected_token` 8/2 (1:9) `;` +
`missing_hex_digit` 9/1 / `missing_digit` 9/1 (1:10).

**a51 — `0xg`** (`var a = 0xg;` + `var b = 0X;`): `expected_token` 8/2 `;`; `missing_hex_digit` 9/1;
`missing_const_final_var_or_type` 10/1 (o `g`); `missing_hex_digit` 22/1 (2:10).

**a28 / b10 / c02 — separadores**
```dart
var a = 1_000;   // sem erro
var b = 100_;    // unexpected_separator_in_number 23/1 (2:9)  — início do número
var c = 0x_1;    // unexpected_separator_in_number 37/1 (3:9)
var d = 1_.5;    // unexpected_separator_in_number 51/1 (4:9)
var e = 1e_5;    // unexpected_separator_in_number 65/1 (5:9)
var g = 1__0;    // sem erro
```
b10: `0x_;` → `unexpected_separator_in_number` 8/1, `integer_literal_out_of_range` 8/3 ("The integer
literal 0x_0 can't be represented in 64 bits."), `missing_hex_digit` 10/1 (o `_`); `1._5` →
`undefined_getter` `_5` 23/2; `1e5_` e `1.5_e3` → `unexpected_separator_in_number` no início de cada um.
c02: `0x__1` → um só `unexpected_separator_in_number` 8/1. b09 (`// @dart=2.19` + `var a = 1_000;`) →
`experiment_not_enabled` 22/5 (2:9) 'digit-separators'.

**a09 / a48 / b20 — `===` e `!==`**
```dart
void f(a, b) {
  a === b;
  a !== b;
}
```
- `unsupported_operator` 19/1 (2:5) The '===' operator is not supported.
- `unsupported_operator` 30/1 (3:5) The '!==' operator is not supported.

a48 (`class C { operator ===(x) => true; }`): só `unsupported_operator` 19/1 (1:20). b20
(`var a = 1 ==== 2;`): `missing_assignable_selector` 8/5, `unsupported_operator` 10/1, `missing_identifier`
13/1 (o 4º `=` é atribuição).

**a10 / b13 / b15 — `(` sem `)`: o scanner põe, o parser move**
```dart
void f() {
  print(1;
}
```
- `expected_token` 20/1 (2:10) Expected to find ')'. — único diagnóstico; no `;` (o `)` sintético nasceu
  antes do `}` da linha 3 e `ensureCloseParen` o moveu)

b13: `print((1 + 2);` → 26/1 no `;`; `print([1, 2);` → `Expected to find ']'` 41/1 no `)`. b15
(`if (a {` … `}`): `Expected to find ')'` 19/1 (2:9) no `{`.

**a11 / b04 — `{` sem `}`: no EOF, além do fim**
```dart
void f() {
  if (true) {
    print(1);
}
```
- `expected_token` 41/1 (5:1) Expected to find '}'. — offset = comprimento do arquivo

b04 (método de classe sem `}`; o `}` da classe fecha o método): `expected_token` 56/1 (7:1) `}`.

**a31 — vários fechos faltando no EOF** (`void f() { g([ (`, sem LF final): `undefined_function` 11/1;
`expected_token` 15/1 (1:16) `;` (parser, no `(`); `expected_token` 16/1 (1:17) `)`, `]`, `}` — três, no
mesmo offset.

**a50 — dois fechos faltando no meio**
```dart
void f() {
  var x = (1 + [2, 3;
  print(x);
}
```
- `argument_type_not_assignable` 26/5 · `expected_token` 31/1 (2:21) `)` · `expected_token` 31/1 (2:21) `]`
  — ambos movidos para o `;`

**a12 — `[ ( ]`** (`var a = [ ( ];`): só `expected_token` 12/1 (1:13) `)` no `]` (opção 1; `()` é record
vazio). **a44 — `[(])]`**: `missing_identifier` 10/1 e `expected_token` 10/1 `)` — os dois do **parser**
(opção 2: o primeiro `]` é ignorado pelo scanner).

**a32 / a43 / b03 / b08** — `[1; }` → `]` 21/1 no `;` (movido). `{'a': [1, 2};` → `]` 30/1 no `}`.
`g(1, [2, {3);` → `]` e `}` 24/1 no `)`. `"${ {1: [2} }"` → `]` 29/1 no `}` interno.

**c07 — fechos não movidos**
```dart
var a = [1, 2;
var b = {1: 2;
var c = (1, 2;
```
- `expected_token` 13/1 (1:14) `]` — scanner, movido para o `;`
- `expected_token` 28/1 (2:14) `}` — parser
- `expected_token` 43/1 (3:14) `;` — parser, no `2`
- `expected_token` 45/1 (4:1) `)` e 45/1 `}` — scanner, no EOF (ninguém os moveu)

**c01 — parser e scanner no mesmo lugar** (`void f(a) { a[1; }`): dois `expected_token` 15/1 (1:16)
`Expected to find ']'.` idênticos.

**a13 / a14 / b01 / b02 — `<`**: `var a = 1 < ;` → só `missing_identifier` 12/1 (parser).
`List<int a;` → `expected_token` 5/3 (1:6) `>` (parser, no `int`) + `not_initialized_non_nullable_variable`.
`void f(a) { a < ( }` → `expected_token` 16/1 `;`, `equality_cannot_be_equality_operand` 18/1,
`missing_identifier` 18/1, `expected_token` 18/1 `)` e **`expected_token` 18/1 `>`** (scanner: `<` abaixo
de `(`). `var a = b < (` no EOF → idem em 13 (`)` e `>` com length 1, os do parser com length 0).

**a29 / a30 — fecho a mais** (sem erro do scanner): `void f() { } }` → `expected_executable` 13/1.
`void f() { print(1)); }` → `expected_token` 18/1 `;`, `missing_identifier` 19/1, `unexpected_token` 19/1.

**a15 / a16 / a17 / a42 — caractere fora do ASCII**
```dart
var a = §;        // a15: illegal_character 8/1 (1:9) Illegal character '167'.  + undefined_identifier 8/1 "Undefined name '§'."
var a = 😀;       // a16: illegal_character 8/1 '55357' · illegal_character 9/1 '56832' · undefined_identifier 8/2
var aé = 1;       // a17: illegal_character 5/1 (1:6) '233' — só isso (identificador fundido)
```
a42: `var piskefløde = 1;` → `illegal_character` 11/1 (1:12) '248'; `var a😀b = 2;` → 25/1 '55357' e 26/1
'56832'; nada mais.

**a18 / a19 / b11 / c03 — espaço não ASCII e controles**: `var a =` + U+00A0 + `1;` → `illegal_character`
7/1 '160' (pulado). `var a = ` + U+0001 + ` 1;` → 8/1 '1'. NUL no início do arquivo → 0/1 '0'. U+001F e
U+007F → `illegal_character` '31' / '127' **mais** `undefined_identifier` (viram identificador).

**a37 — BOM**: BOM + `var a = §;` → `illegal_character` 8/1 (1:9), igual a a15 (o BOM não conta).

**a46 — U+FFFD fora de string**: o servidor de análise cai com `UnimplementedError: Encoding "null"` em
`translateErrorToken` (`errors.dart:90`); o `dart analyze` não devolve JSON.

**a36 / a57 — `#!`**: na linha 2 → `expected_executable` 11/1 e 12/1, `expected_token` 13/7,
`missing_const_final_var_or_type` 13/7. Na linha 1 → sem erro (com `0x;` na linha 2: `missing_hex_digit`
29/1).

**a54 — `\` e crase fora de string**: sem `illegal_character`; só erros do parser (`expected_token` 8/1,
`expected_executable` 10/1 e 12/1, `unexpected_token` 13/1 …).

#### E.1.12 No DartForge — regra a regra

`L` = `crates/frontend/src/lexer.rs`; `M` = `crates/frontend/src/parser/mod.rs`; `X` =
`crates/frontend/src/parser/expressions.rs`; `F` = `crates/frontend/src/parser/fasta.rs`.

| regra do fasta | hoje | diferença |
|---|---|---|
| erros não param a varredura; ErrorToken na cabeça; relato no fim | `lex` devolve `Err` no 1º erro (`L:50-60`); `parse_lexed_com` devolve unidade vazia (`M:150-158`) | arquivo inteiro perdido depois do 1º erro léxico; nenhuma cascata do oráculo sai |
| offsets em UTF-16, length 1 | spans em bytes, `[inicio, inicio + 1)` (`L:92-94`) | caractere multibyte → length 0 depois da conversão (`paridade/src/json.rs:108-110`) |
| BOM removido antes do scanner | o lexer não trata BOM (cairia em `b >= 0x80`, `L:192`); só `features.rs:271-273` o pula ao procurar `@dart` | se o carregador remove o BOM antes: não verificado |
| `#!` só no offset 0, até LF/CR | `L:118-124`, até LF | CR isolado |
| espaço = espaço, TAB, LF, CR | `is_ascii_whitespace` (`L:129`) | FF (`0x0C`) aceito aqui, `illegal_character '12'` lá |
| comentário de bloco aninhado; sem fecho → erro em `fim - 1` e fim | `block_comment` (`L:221-241`), mesma posição | aborta (o resultado prático coincide: nada depois do `/*` existe) mas a unidade antes do `/*` é perdida |
| palavras-chave: todas viram `KeywordToken`, com flags por recurso | só reservadas (`Keyword::from_text`, `L:164`); embutidas e pseudo por texto (`F:34-47`) | equivalente por construção; `extension`/`late`/`required`/`augment` sem flag: não verificado |
| `$` em identificador | `is_ident_start`/`is_ident_part` (`L:62-68`) | igual |
| `0x` sem dígito → HEX sintético + `MISSING_HEX_DIGIT` | `L:251` exige dígito ou `_` → `Int(0)` + identificador | FN + FP (E.1.10) |
| expoente sem dígito → DOUBLE sintético + `MISSING_DIGIT` | `L:268-274`, `L:304` (`expoente_valido`, `L:284-296`) → número + identificador `e` | FN + FP |
| separadores: token único, erro no início do número | tokens iguais (`L:249-317`); erro em `conferir_separadores` (`X:201-229`) | equivalente nos casos de E.1.11; `0x_` (sem dígito) diverge pela linha acima |
| `.5` DOUBLE; `1.` = INT + `.` | `L:174`, `L:275` | igual |
| string de uma linha: LF, CR ou EOF → token sintético + erro em `fim - 1` | `L:344-346`, `L:359-361`, posição em `L:105-115` | aborta; CR isolado não termina |
| `\` consome um caractere, mas a quebra depois dela termina a string | `L:362-369` | igual |
| multilinha/crua sem fecho → EOF | `L:344-346` | aborta |
| `$nome` com `allowDollar: false`; palavra-chave vira `KeywordToken` | `L:391-423` (nome sem `$`; sempre `Kind::Ident`) | o parser trata a palavra-chave (não conferido nesta parte) |
| `$` sem nome → identificador sintético + `MISSING_IDENTIFIER` depois do `$` | `L:424-442` + `X:1822-1829` | equivalente |
| `${`: grupo na pilha; `}` casado devolve `$STX`; EOF → `discardInterpolation` | pilha `suspended` com contagem de `{`/`}` (`L:178-191`, `L:371-390`) | sem erro léxico para `${` aberto no EOF; `[`/`(` abertos dentro de `${` não são fechados pelo `}` |
| escapes validados no AstBuilder; offset `token + índice no trecho sem aspa` | `erro_de_escape` (`X:98-170`), relato em `X:1888-1909` sempre na `\` | posição diferente em multilinha (−(`firstQuoteLength` − 1)) e em trechos depois de interpolação (+1) |
| `===`/`!==`: um token + `UNSUPPORTED_OPERATOR` | ausentes da tabela (`L:463-517`) | FN + FP |
| `>`: `>=`, `>>`, `>>=`, `>>>`, `>>>=` tokens inteiros (com flag) | `>` sempre sozinho + `glued` (`L:5-6`, `L:508`); o parser compõe | equivalente para código válido; `>>>` em biblioteca < 2.14: não verificado |
| `[]`, `[]=` tokens únicos | `[` e `]` separados | sem efeito em diagnósticos conhecidos |
| crase e `\` são tokens | `caractere_ilegal` (`L:525-527`) | FP `illegal_character` e aborto, em vez dos erros do parser |
| caractere ilegal: três classes, fusão de identificador, um erro por unidade UTF-16 | `L:192-198`, `L:525-527`: erro e aborto | sem fusão; fora do BMP um erro com o ponto de código; length em bytes |
| pilha de grupos com fechos sintéticos e duas opções de recuperação | não existe; `matching_close` (`M:701-719`) conta profundidade sem olhar o tipo | `( ]` casa; sem fecho sintético; sem posição "movida" |
| `<`/`>` como grupo, `discardOpenLt` | `fim_do_grupo_lt` (`F:216-284`), porte parcial: fecho sem abridor trunca a pilha por `rposition`, sem as opções 1/2 | diverge quando há fecho de outro tipo no meio |
| `EXPECTED_TOKEN` do scanner para fecho que falta | só no `Eof` (`M:505-508`) | fecho que falta no meio do arquivo sai como erro do parser, noutro lugar ou não sai |

Ordem de implementação sugerida pelos dados: (1) lexer que não aborta, devolvendo tokens + diagnósticos
(strings sem fecho, comentário sem fecho, `0x`, expoente, `===`/`!==`, caractere ilegal com fusão, crase e
`\` como tokens) — cobre as 15 FN de `unterminated_string_literal`, `unsupported_operator`,
`missing_digit`, `missing_hex_digit`, `invalid_unicode_escape_started` e os FP associados de
`expected_token`/`missing_identifier`/`undefined_identifier`; (2) span do caractere inteiro em
`illegal_character`; (3) pilha de grupos com fecho sintético e `endGroup`, consumida pelo parser.

#### E.1.13 Correções ao texto da rodada 3

- **T3 (§E.0):** "comprimento sempre 1 (`ErrorToken.length`, `fe:scanner/error_token.dart:73`)" — o
  comprimento 1 vem de `FastaErrorReporter.reportScannerError` (`an:fasta/error_converter.dart:578-588`),
  não do token (`UnterminatedString.length` é `endOffset - charOffset`, :194-195). "O parser pula os
  ErrorToken" — pula no início e os **relata no fim** de `parseUnit` (:440), depois de poder mover os
  fechos sintéticos. Falta no T3: `UNEXPECTED_SEPARATOR_IN_NUMBER`, `EXPECTED_TOKEN` com `'>'`,
  `MISSING_IDENTIFIER` de `$`, e o `U+FFFD` que derruba o analyzer.
- **§SCANNER:** "`lex_recuperando` … código JÁ escrito, compilação pendente" — **não está no disco**:
  `crates/frontend/src/lexer.rs` só tem `lex` (aborta no 1º erro), `parse_lexed_com` devolve unidade
  vazia, e `E:\dftemp\analise\wip-2026-10-02.patch` não contém `lex_recuperando` nem arquivos de
  `crates/frontend`. A frase "`lex()` … agora também [para] em `0x`, `1e`, `===`" também não vale: hoje
  `0x;`, `1e;` e `===` **não** são erro léxico (viram outros tokens e geram FP no parser).
- **`expected_token` (§E):** "scanner … para fecho que falta no EOF" — também no meio do arquivo (fecho de
  outro tipo) e para `<` abaixo de outro abridor; quando `ensureCloseParen` move o `)` sintético, o
  parser **não** relata nada (o único diagnóstico é o do scanner).
- **Cauda (§E):** "`illegal_character` (pos 1: UTF-16 × byte?)" — a linha:coluna coincide; a diferença é o
  **comprimento** (span de 1 byte dentro de `ø`). "`invalid_unicode_escape_started` … §SCANNER" — o código
  é do AstBuilder (`quote.dart`), não do scanner; só o pré-requisito (não abortar na string sem fecho) é
  do lexer.

#### E.1.14 Não verificado

- Onde o analyzer elimina diagnósticos idênticos (c02) e o critério de desempate do JSON do `dart analyze`.
- A remoção do BOM por `utf8.decode` (fonte de `dart:convert` não aberta; comportamento confirmado em a37).
- `_isAtEnd` verdadeiro: nenhum exemplo o exercita; a leitura do fonte indica que exige um arquivo sem
  nenhum token real.
- Ramo `!hasDigit` de `tokenizeFractionPart` (:1457-1470): dado como inalcançável só pela leitura.
- Scanner com `enableTripleShift`/`enableNonNullable`/`enableExtensionMethods` desligados (bibliotecas
  antigas) e `forAugmentationLibrary` ligado: não rodados no oráculo.
- Lado DartForge: tratamento de BOM no carregamento; palavra-chave depois de `$` no parser; amostras de
  `missing_identifier` do placar não foram atribuídas uma a uma ao caminho `$` sem nome.
- `Utf8BytesScanner` (não usado pelo analyzer) não foi lido além dos cabeçalhos.

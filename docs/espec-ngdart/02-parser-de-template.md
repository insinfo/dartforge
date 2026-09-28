# 02 — Parser de template (tokenização, AST, desaçúcar, espaços, seletores, esquema DOM)

Especificação derivada da **leitura** do `ngast-3.0.0-dev.2` e do `ngcompiler-3.0.0-dev.3`.
Nada aqui foi conferido executando o compilador: todos os exemplos são **(derivado)**, isto
é, deduzidos só do código citado. Abreviações usadas nas citações:

| Abreviação | Arquivo |
|---|---|
| `ST` | `ngast/lib/src/simple_tokenizer.dart` |
| `SC` | `ngast/lib/src/scanner.dart` |
| `TK` | `ngast/lib/src/token/tokens.dart` |
| `CH` | `ngast/lib/src/token/chars.dart` (`namedEntities`) |
| `NP` | `ngast/lib/src/parser.dart` (`NgParser`) |
| `RP` | `ngast/lib/src/parser/recursive.dart` (`RecursiveAstParser`) |
| `AST:x` | `ngast/lib/src/ast/x.dart` |
| `DS` | `ngast/lib/src/visitors/desugar.dart` |
| `WS` | `ngast/lib/src/visitors/whitespace.dart` |
| `RV` | `ngast/lib/src/visitors/recursive.dart` |
| `MI` | `ngast/lib/src/expression/micro.dart` |
| `MP` | `ngast/lib/src/expression/micro/parser.dart` |
| `MS` | `ngast/lib/src/expression/micro/scanner.dart` |
| `EX` | `ngast/lib/src/exception_handler/exceptions.dart` |
| `ATP` | `ngcompiler/lib/v1/src/compiler/template_parser/ast_template_parser.dart` |
| `TP` | `ngcompiler/lib/v1/src/compiler/template_parser.dart` |
| `MDV` | `ngcompiler/lib/v1/src/compiler/template_parser/missing_directives_validator.dart` |
| `SEL` | `ngcompiler/lib/v1/src/compiler/selector.dart` |
| `AM` | `ngcompiler/lib/v1/src/compiler/attribute_matcher.dart` |
| `DSR` | `ngcompiler/lib/v1/src/compiler/schema/dom_element_schema_registry.dart` |
| `SSV` | `ngcompiler/lib/v1/src/compiler/schema/skip_selectors_validator.dart` |
| `HT` | `ngcompiler/lib/v1/src/compiler/html_tags.dart` |
| `ADN` | `ngcompiler/lib/v1/src/compiler/ast_directive_normalizer.dart` |
| `PU` | `ngcompiler/lib/v1/src/compiler/parse_util.dart` |
| `ECV` | `ngcompiler/lib/v1/src/compiler/view_compiler/expression_converter.dart` |
| `PP` | `ngcompiler/lib/v1/src/compiler/provider_parser.dart` |

Raízes: `ngast` = `/root/.pub-cache/hosted/pub.dev/ngast-3.0.0-dev.2/`, `ngcompiler` =
`/root/.pub-cache/hosted/pub.dev/ngcompiler-3.0.0-dev.3/`.

Porte Rust (`crates/gerador_ng/src/`): `html.rs` (`H`), `micro.rs` (`M`), `seletor.rs`
(`S`), `entidades.rs` (`E`), `dom.rs` (`D`), `visao.rs` (`V`), `componente.rs` (`C`). Como
`visao.rs` está em edição, as citações dele são por **nome de função**; as de `html.rs`,
`micro.rs` e `seletor.rs` também trazem a linha.

Legenda da cobertura: **COBERTO** (o porte faz o mesmo), **PARCIAL** (faz parte, ou faz
diferente em casos de borda), **RECUSA** (o porte não emite e recusa o componente — seguro),
**FALTA** (o porte não faz e segue adiante — pode gerar saída diferente ou aceitar o que o
oficial rejeita).

---

## 0. Visão geral do pipeline

### R0.1 — Duas passagens de parse sobre o mesmo texto
1. **Normalização da diretiva** (`ADN:190-214`): `ast.parse(template, desugar: false,
   exceptionHandler: AstExceptionHandler)` só para coletar os `ngContentSelectors`, na ordem
   de documento, com `_FindAllNgContentSelectors` (`ADN:217-235`). `<ng-content select>` sem
   valor lança `BuildError` *"The "select" attribute must have a value or be omitted"*
   (`ADN:226-231`). Como é `desugar: false`, um `<ng-content>` sob `*ngIf` também conta.
2. **Parse do template** (`ATP:59-108`), em fases, cada uma só roda se a anterior não deixou
   erro (`throwErrorsIfAny`/`throwRecoverableErrors`):
   1. `ast.parse(template, exceptionHandler: AstExceptionHandler, sourceUrl)` — com
      `desugar: true` (padrão de `NP:148-175`). Lista vazia → `const []` (`ATP:81`).
   2. `_processRawTemplateNodes` (`ATP:110-124`): `_NamespaceVisitor` →
      `_ElementFilter` → `MinimizeWhitespaceVisitor` **ou** `_PreserveWhitespaceVisitor` →
      `_TemplateValidator`. A flag `forceMinifyWhitespace` força a minimização (`ATP:116-119`).
   3. `_bindDirectivesAndProviders` (`ATP:126-142`): `_BindDirectivesVisitor` (ngast →
      `ng.*`), `_OnPushValidator` se o componente é OnPush, `_ProviderVisitor`, e o
      `MissingDirectiveValidator` quando `CompileContext.current.validateMissingDirectives`
      (verdadeiro por padrão; ver R11).
   4. `_processBoundTemplateNodes` (`ATP:144-153`): otimização, `_SortInputsVisitor`,
      `_PipeValidator`.

`AstExceptionHandler` estende `RecoveringExceptionHandler` (`PU:8-42`): o scanner roda em
modo de recuperação, junta **todas** as exceções e `throwErrorsIfAny` lança um
`BuildError.fromMultiple` com a mensagem de cada `ParserErrorCode` e o título
*"Errors in $url while compiling component $name"* (`PU:27-41`). Qualquer erro do ngast é,
portanto, fatal.

**Porte:** `H:analisar` (`html.rs:151`) faz um parse só, tolerante, sem acusar erro nenhum
(o comentário em `html.rs:148` diz isso), e já aplica a minimização (`html.rs:157`). A
coleta de `ngContentSelectors` é `H:projecoes` (`html.rs:133`). — **PARCIAL** (ver as
lacunas de cada regra).

---

## 1. Tokenização, nível 1 — `NgSimpleScanner` (`ST`)

### R1.1 — Estados e ordem
Estados: `doctype` (inicial), `text`, `element`, `comment`, `commentEnd`, `interpolation`
(`ST:379-386`). O tokenizador emite tokens até `eof` e emite o `eof` explicitamente
(`ST:14-23`).

### R1.2 — DOCTYPE vira **texto**
No estado inicial (`ST:116-137`): se o texto começa com `<!DOCTYPE` (maiúsculas, exato) ou
com `>` (regex `(<!DOCTYPE)|(>)` ancorada na posição 0), emite **um token de texto** do
início até o primeiro `>` inclusive (ou até o fim). Caso contrário cai em `scanText`.
- Ex.: `<!DOCTYPE html><p>x</p>` → `TextAst("<!DOCTYPE html>")`, `ElementAst(p)`.
- `<!doctype html>` (minúsculo) não casa; o `<` inicia tag e `!` gera erro.

**Porte:** `H:nos` (`html.rs:263-270`) pula qualquer `<!…>` (não gera texto) em qualquer
posição. — **PARCIAL** (difere: oficial gera `TextAst`; e `<!x` fora do início é erro no
oficial).

### R1.3 — Texto (`ST:241-292`)
Regex `([^\<]+)|(<!--)|(<)` na posição atual:
- grupo 1: texto até o próximo `<`. Dentro dele procura `{{` ou `}}` (`({{)|(}})`); se houver
  texto antes, devolve o texto; senão devolve `mustacheBegin` (e vai ao estado
  `interpolation`) ou `mustacheEnd`.
- grupo 2 `<!--`: `commentBegin`, estado `comment`.
- grupo 3 `<`: `closeTagStart` se seguido de `/` (consome), senão `openTagStart`; estado
  `element`.

Consequência: **todo `<` no texto abre tag**. `a < b` é erro (`elementIdentifier`), não
texto.

**Porte:** `H:nos`/`H:ler_texto` (`html.rs:247`, `319`) só tratam `<` como tag se seguido
de letra ou `!`; senão chamam `ler_texto`, que **não avança** quando o caractere atual já é
`<` (`html.rs:321`): `a < b`, `<3`, `{{ a < b }}` fazem `H:nos` repetir o mesmo estado
indefinidamente. — **FALTA (defeito: laço infinito)**.

### R1.4 — Interpolação (`ST:294-350`)
Estado separado para que `<` dentro de `{{ }}` não abra tag. Procura o próximo `{{`/`}}`:
- achou com texto antes → token de texto (o valor da interpolação);
- `}}` → `mustacheEnd`, volta a `text`; `{{` → outro `mustacheBegin` (erro adiante);
- não achou: se há `\n`, devolve o texto até o `\n` e depois o `\n` como *whitespace*,
  voltando ao estado `text` (o `{{` fica sem fechar → `unterminatedMustache`); senão texto
  até o fim.

**Porte:** `H:ler_texto` (`html.rs:319-351`) corta o texto no primeiro `<` **antes** de
procurar `}}`, logo `{{ a<b }}` quebra (ver R1.3). `{{` sem `}}` vira texto **sem** o `{{`
(`html.rs:330,343`) — o oficial acusa erro. `}}` solto vira texto — o oficial acusa
`unopenedMustache`. — **PARCIAL**.

### R1.5 — Decodificação de entidades: onde se aplica
`_newTextToken` (`ST:356-357`) chama `_unEscapeText` em **todo** token de texto do
tokenizador simples: texto comum, **valor da interpolação** (`ST:318,329,337`), **conteúdo
de comentário** (`ST:102,106`) e o DOCTYPE. **Não** se aplica a valores de atributo (tokens
de aspas, `ST:191-200`).

Regex (`ST:55-57`), aplicada com `replaceAllMapped`, da esquerda para a direita:
`&#([0-9]{2,4});` | `&#x([0-9A-Fa-f]{2,4});` | `&([a-zA-Z]+);`.
- decimal/hex → `String.fromCharCode(n)` (`ST:362-367`); com 2–4 dígitos. `&#9;` (1 dígito)
  e `&#12345;` (5) **não** decodificam. `&#X41;` (X maiúsculo) não decodifica.
- nomeada → `namedEntities[nome] ?? nome` (`ST:370-372`): **nome desconhecido vira o próprio
  nome sem `&` e `;`** (`&foo;` → `foo`).
- Ex.: `a &amp; b` → `a & b`; `&#x41;&#65;` → `AA`; `&ngsp;` → `U+E500`; `&nbsp;` →
  `U+00A0`; `{{ a &amp;&amp; b }}` → valor da interpolação `a && b`.

**Porte:** `H:decodificar`/`H:entidade_em` (`html.rs:544`, `572`) reproduzem a regex e o
`?? nome`; código em faixa de substituto vira `U+FFFD` (o Dart criaria um substituto solto).
Tabela `E:ENTIDADES` tem as **253** entradas de `CH:5-260`, iguais (conferido por script), em
ordem para busca binária. Mas o porte decodifica **só o texto**: o valor de
`No::Interpolacao` (`html.rs:334-338`) não passa por `decodificar`. — **PARCIAL**
(`{{ a &amp;&amp; b }}` diverge).

### R1.6 — Dentro da tag (`ST:31-51`, `139-239`)
Regex de alternativas (a primeira que casa na posição):

| grupo | padrão | token |
|---|---|---|
| 1 | `]` | `closeBracket` |
| 2 | `!` | `bang` |
| 3 | `-` | `dash` |
| 4 | `)` | `closeBanana` se seguido de `]` (consome), senão `closeParen` |
| 5 | `>` | `tagEnd` (volta a `text`) |
| 6 | `/` | `voidCloseTag` se seguido de `>` (consome; volta a `text`), senão `forwardSlash` |
| 7 | `[` | `openBanana` se seguido de `(` (consome), senão `openBracket` |
| 8 | `(` | `openParen` |
| 9 | `\s+` | `whitespace` |
| 10 | `[a-zA-Z]([\w\_\-:])*[a-zA-Z0-9]?` | `identifier` |
| 12 | `"([^"\\]+\|\\.)*"?` | aspas duplas; lexema com `\"` → `"`; fechado se o último char é `"` |
| 14 | `'([^'\\]+\|\\.)*'?` | idem, aspas simples, `\'` → `'` |
| 16 | `<` | `commentBegin` se `<!--`; `closeTagStart` se `</`; senão `openTagStart` |
| 17–23 | `=` `*` `#` `.` `%` `\` `@` | `equalSign`, `star`, `hash`, `period`, `percent`, `backSlash`, `atSign` |
| — | outro | `unexpectedChar` |

Observações:
- Identificador **começa com letra ASCII**; `_x` ou `1x` como nome de atributo/tag é erro.
- Valor entre aspas aceita escape `\` de qualquer caractere, mas só `\"`/`\'` é trocado;
  `\n` fica como os dois caracteres. O conteúdo (`contentLexeme`) é o lexema sem as aspas
  (`TK:211-224`).
- Não há decodificação de entidade no valor do atributo.

**Porte:** `H:ler_elemento` (`html.rs:353`) lê o nome da tag até espaço, `>` ou `/`;
`H:ler_atributo` (`html.rs:417`) lê o nome até espaço, `=`, `>` ou `/`, aceita valor **sem
aspas** e termina o valor na primeira aspa igual (sem escape `\`). — **PARCIAL** (aceita o
que o oficial rejeita; `\"` diverge).

### R1.7 — Comentário (`ST:88-114`)
Depois de `<!--`, tudo até `-->` é um token de texto (decodificado, R1.5); sem `-->` o texto
vai até o fim e o scanner acusa `unterminatedComment` (`SC:170-182`, `492-508`).

**Porte:** `H:ler_comentario` (`html.rs:290`). Comentário sem fim é aceito. — **PARCIAL**
(o comentário some no `ng.*` de qualquer forma, `ATP:671-673`; ver R6.3 sobre o papel dele
na minimização).

---

## 2. Tokenização, nível 2 — `NgScanner` (`SC`)

### R2.1 — Máquina de estados e junção de texto
`NgScanner.scan` (`SC:54-167`) transforma os tokens simples em `NgToken`. `scanText`
(`SC:1128-1154`) **junta** tokens de texto e *whitespace* consecutivos num só
`NgToken.text` (ex.: o `\n` que o estado de interpolação devolve à parte).

### R2.2 — Decoradores (atributos) e prefixos
Em `scanElementDecorator` (`SC:528-596`):

| primeiro token | resultado |
|---|---|
| `identifier` | nome composto: junta `period`, `identifier`, `dash`, `percent`, `backSlash` seguidos (`_scanCompoundDecorator`, `SC:511-525`) |
| `(` | `eventPrefix`, depois nome composto, exige `)` (`suffixEvent`) |
| `[` | `propertyPrefix`, nome composto, exige `]` (`suffixProperty`) |
| `[(` | `bananaPrefix`, nome composto, exige `)]` (`suffixBanana`) |
| `#` | `referencePrefix`, **um** `identifier` (não composto, `SC:875-901`) |
| `*` | `templatePrefix`, **um** `identifier` |
| `@` | `annotationPrefix`, nome composto (`SC:904-934`) |

Ex.: `(keyup.enter)` → nome `keyup.enter`; `[style.width.%]` → `style.width.%`;
`[attr.aria-label]` → `attr.aria-label`; `@i18n:title` → `i18n:title` (o `:` faz parte do
`identifier`). `#a.b` → erro (`#` só aceita um `identifier`, o `.` seguinte cai em
`expectedWhitespaceBeforeNewDecorator`).

- Depois de um decorador: `=` (com espaços opcionais antes/depois, `RP:152-158`), `>`, `/>`,
  ou espaço seguido de novo decorador. Dois decoradores colados (`a[b]`) →
  `expectedWhitespaceBeforeNewDecorator` (`SC:209-224`).
- Valor exige aspas: `identifier` depois do `=` → `elementDecoratorValueMissingQuotes`
  (`SC:656-662`); aspa não fechada → `enclosedQuote` (`SC:614-633`).

**Porte:** `H:classificar` (`html.rs:467-534`) decide pelo **texto** do nome já lido
(`[(`…`)]`, `[`…`]`, `(`…`)`, `#`, `*`, `@`), não por tokens. Casos válidos batem; os
inválidos (`#a.b`, `a[b]`, valor sem aspas) são aceitos. — **PARCIAL**.

### R2.3 — Erros do scanner
Ver o catálogo completo em R12.1. Os de uso mais comum: `expectedTagClose`,
`elementIdentifier`, `expectedWhitespaceBeforeNewDecorator`, `unterminatedMustache`,
`unopenedMustache`, `emptyInterpolation` (`{{}}`, `SC:860-866`), `suffixEvent`,
`suffixProperty`, `suffixBanana`, `voidCloseInCloseTag` (`</br/>`), `enclosedQuote`.

**Porte:** nenhum. — **FALTA**.

---

## 3. Parser recursivo (`RP`, `NP`)

### R3.1 — Elementos vazios e SVG
- Vazios (lista exata, `NP:14-31`, comparação **sensível a maiúsculas**, `RP:326`):
  `area, base, br, col, command, embed, hr, img, input, keygen, link, meta, param, source,
  track, wbr`.
- SVG (`NP:42-137`, para aceitar `/>`): `altGlyph … vkern` (lista no fonte). A checagem é
  `_svgElements.contains(nome.replaceAll('svg:', ''))` (`RP:328-329`). **`svg` não está na
  lista**: `<svg/>` é erro.
- Vazio: não procura filhos nem fechamento; `ElementAst.isVoidElement` ⇔
  `closeComplement == null` (`AST:element:223`).
- `/>` em elemento não vazio e não SVG → `nonVoidElementUsingVoidEnd`, e o parser segue
  como se fosse `>` (`RP:411-423`).
- `</br>` (fechamento de vazio) → `voidElementInCloseTag` (`RP:47-53`).

**Porte:** `H:VAZIOS` (`html.rs:13-16`) **não tem** `command` nem `keygen`, e compara em
minúsculas (`html.rs:398`); `/>` fecha **qualquer** elemento (`html.rs:380-384`). —
**PARCIAL**.

### R3.2 — Nó por tipo de nome
`parseElement` (`RP:313-469`):
- `ng-container` (exato) → `parseContainer` (`RP:85-123`): aceita só `@anotação` e `*estrela`
  (no máximo uma, `_addStarAst`, `RP:761-771`); qualquer outro decorador →
  `invalidDecoratorInNgContainer` *"Only '*' bindings are supported on <ng-container>"*.
  `/>` → `nonVoidElementUsingVoidEnd` (`RP:855-870`).
- `ng-content` (exato) → `parseEmbeddedContent` (R3.5).
- `template` (**comparado em minúsculas**, `RP:323`) → `EmbeddedTemplateAst`.
- resto → `ElementAst`.

**Porte:** tudo vira `No::Elemento` exceto `ng-content`, reconhecido sem distinguir
maiúsculas (`html.rs:403`). As restrições de `<ng-container>` não são checadas no parse. O
tratamento de `<template>` e `<ng-container>` está em `V:template_como_container`. —
**PARCIAL**.

### R3.3 — Classificação dos decoradores num elemento
`parseDecorator` (`RP:127-308`) produz, pela forma:

| forma | AST | observações |
|---|---|---|
| `[(x)]="v"` | `BananaAst(x, v)` | |
| `(x)="v"` | `EventAst(name, reductions)` | `name` = parte antes do 1º `.`; `reductions` = resto (`AST:event:114-155`) |
| `[x]="v"` | `PropertyAst(name, postfix, unit)` | `name`/`postfix`/`unit` = partes 0/1/2 por `.` (`AST:property:133-179`); mais de 3 partes → `propertyNameTooManyFixes` (`RP:192-198`) |
| `#x` / `#x="y"` | `ReferenceAst(variable: x, identifier: y)` | (`AST:reference:130-134`) |
| `*x="v"` | `StarAst(x, v)` | |
| `@x` / `@x="v"` | `AnnotationAst(x, v)` | |
| `on-x="v"` | `EventAst` | nome vazio (`on-`) → `elementDecoratorAfterPrefix` (`RP:237-258`) |
| `bind-x="v"` | `PropertyAst` | idem (`RP:259-280`) |
| `let-x="v"` | `LetBindingAst(x, v)` | (`RP:281-294`) |
| outro | `AttributeAst(name, value, mustaches)` | `mustaches` = `{{…}}` achados no valor (R3.4) |

`value` é sempre o conteúdo cru entre aspas (`valueToken.innerValue.lexeme`); **atributo
sem `=` tem `value == null`**, distinto de `x=""` (valor `""`).

Regras por elemento (`RP:343-409`):
- `let-` fora de `<template>` → `invalidLetBindingInNoTemplate`; `let-` com nome vazio →
  `elementDecoratorAfterPrefix`.
- `*` em `<template>` → `invalidDecoratorInTemplate`; segundo `*` no mesmo elemento →
  `duplicateStarDirective` (o segundo é descartado).
- `[(x)]` em `<template>` → `invalidDecoratorInTemplate`.
- A ordem das listas (`attributes`, `properties`, `events`, `references`, `bananas`,
  `annotations`, `letBindings`) é a ordem de escrita **dentro de cada tipo**.

**Porte:** `H:classificar` (`html.rs:467-534`) separa as mesmas listas na mesma ordem.
Diferenças: `let-x` vai para `atributos` (em qualquer elemento, sem erro); `on-`/`bind-`
vazios viram atributo comum; segundo `*` substitui o primeiro (`el.estrela = Some(...)`);
não distingue `x` de `x=""` (ambos com `valor == ""`, `html.rs:437`); `ng-content` perde
`ngProjectAs` e `#ref` (R3.5). — **PARCIAL**.

### R3.4 — `{{ }}` em valor de atributo simples
`_parseMustacheInPlainAttributeValue` (`RP:597-695`) varre `({{)|(}})` no valor cru e
produz `InterpolationAst`s só para marcar que há interpolação; erros: `{{` repetido antes de
`}}` e `{{` pendente no fim → `unterminatedMustache`; `}}` sem `{{` → `unopenedMustache`.
Só `AttributeAst` (atributo simples) tem `mustaches`; `[x]="{{a}}"` não.

**Porte:** o critério "tem interpolação" é `valor.contains("{{")` (`H:Elemento::liga_propriedade`,
`html.rs:111-113`; `V:atributo_interpolado`). — **PARCIAL** (sem os erros).

### R3.5 — `<ng-content>`
`parseEmbeddedContent` (`RP:472-591`):
- Aceita só `select` (atributo), `ngProjectAs` (atributo) e **um** `#ref` **sem valor**.
  Duplicatas → `duplicateSelectDecorator` / `duplicateProjectAsDecorator` /
  `duplicateReferenceDecorator`; `#r="x"` → `referenceIdentifierFound`; outro →
  `invalidDecoratorInNgContent`.
- Depois do `>`, um texto **só de espaços** é engolido; qualquer outra coisa antes de
  `</ng-content>` → `ngContentMustCLoseImmediately` (`RP:546-580`). `<ng-content/>` →
  `nonVoidElementUsingVoidEnd`.
- `selector` (`AST:content:116-123`): sem `select` → `'*'`; `select` sem valor → `null`
  (erro em `ADN`, R0.1); senão o valor cru.
- `ngProjectAs` → valor do atributo.

**Porte:** `No::Conteudo { seletor }` (`html.rs:44-47`, `403-407`): `select` sem valor vira
`Some("")` (oficial: erro); `ngProjectAs` e `#ref` **descartados em silêncio**; filhos são
lidos e jogados fora. — **PARCIAL/FALTA** (`ngProjectAs` muda o `ngContentIndex`, R8.6).

### R3.6 — Fechamento de tags
`_parseCloseElement` (`RP:776-852`) compara o nome de fechamento **exatamente** (sensível a
maiúsculas) com o de abertura. Fechamento de outro nome:
- se esse nome está na pilha de tags abertas → fecha o atual sinteticamente e acusa
  `cannotFindMatchingClose`;
- senão → cria um nó sintético pareado com o fechamento solto (`ContainerAst`,
  `EmbeddedContentAst`, `EmbeddedTemplateAst` ou `ElementAst`) e acusa
  `danglingCloseElement`.
- Fim do arquivo sem fechar → `cannotFindMatchingClose`.

**Porte:** `H:nos` (`html.rs:250-257`) fecha com `eq_ignore_ascii_case` e **ignora**
fechamentos de outros nomes; fim de arquivo fecha tudo. — **PARCIAL** (sem erros; `<A></a>`
casa no porte e não no oficial).

---

## 4. Desaçúcar (`DesugarVisitor`, `DS`)

Roda de baixo para cima: visita os filhos primeiro (`DS:42-50`), depois o nó.

### R4.1 — `[(x)]="v"` (`DS:76-92`, só em `ElementAst`)
Para cada banana, em ordem, **se `value != null`**: acrescenta ao **fim** de `events`
`EventAst('${x}Change', '${v} = $event')` e ao **fim** de `properties` `PropertyAst(x, v)`.
Banana sem valor é **descartada em silêncio** (`continue`). Depois `bananas.clear()`.
- Ex.: `<x (a)="f()" [(val)]="m" [b]="1">` → props `[b, val]`, events `[a, valChange]`,
  `valChange` com valor `m = $event`.
- O valor é colado como texto: `[(v)]="a.b"` → `a.b = $event`.

**Porte:** feito sob demanda em `V` (no elemento de componente filho e no de diretiva:
propriedade ao fim, evento `x = $event` ao fim — ver os blocos com `format!("{}Change", …)`
em `V`), e em `S:Elemento::do_template` (`seletor.rs:355-373`) para o casamento. Banana sem
valor não é descartada (não há como distinguir, R3.3). `[(x)]` em elemento HTML sem
diretiva é recusado. — **PARCIAL**.

### R4.2 — `*dir="expr"` (`DS:94-163`)
Aplica-se a `ElementAst` e a `ContainerAst` (`DS:20-40`; em elemento, **depois** das
bananas). Com `starExpression = value?.trim()`:
1. Se `isMicroExpression(starExpression)` (R4.3): parseia a microssintaxe; as propriedades e
   `let`s produzidos vão ao novo `<template>`; **se nenhuma propriedade tem nome == `dir`,
   acrescenta o atributo `dir` sem valor** (`DS:125-127`). Exceção do parser → o erro é
   reportado e o nó original volta **sem** desaçúcar (`DS:116-119`).
2. Senão, `starExpression == null` (sem `=`) → atributo `dir` sem valor; senão →
   `PropertyAst(dir, starExpression)` — **mesmo se for `""`**.
3. O resultado é `EmbeddedTemplateAst.from(origin: star, childNodes: [nó original],
   attributes, properties, letBindings)`; `stars.clear()`.

Exemplos:
- `<p *ngIf="c">` → `<template [ngIf]="c"><p></p></template>`.
- `<li *ngFor="let x of xs; let i = index">` → `<template ngFor let-x let-i="index"
  [ngForOf]="xs">` (atributo `ngFor` porque nenhuma propriedade se chama `ngFor`).
- `<p *foo>` → `<template foo>`; `<p *foo="">` → `<template [foo]="">`.

**Porte:** `V:template_da_estrela` (atributo `dir` quando a **primeira** propriedade não é
`dir` — equivalente, pois só a primeira pode ser a implícita; `let`s viram atributos
`let-x`), `V:template_como_container`, `V:guarda_do_template` (descrição do `<template>` com
`dir` sem valor), `M:analisar` (`micro.rs:35`). `*foo=""` vira atributo (oficial:
propriedade vazia). — **PARCIAL**.

### R4.3 — `isMicroExpression` (`MI:7-16`)
Verdadeiro se a expressão (já aparada) **começa com `let`** (inclui `letter`!) ou casa
`\S+[:;]` **ancorado no início** (uma sequência sem espaço seguida de `:` ou `;` — com
retrocesso, basta haver `:`/`;` antes do primeiro espaço, fora da posição 0).
- `cond; else: t` → micro; `a ? b : c` → não (`a` seguido de espaço); `x.y:z` → micro;
  `'a:b'` → micro (!).

**Porte:** `M:e_micro` (`micro.rs:133-141`). — **COBERTO** (diferença só em `\s` do Dart
versus `char::is_whitespace`, R6.6).

### R4.4 — Scanner da microssintaxe (`MS`)
A string recebe à esquerda `' ' * expressionOffset` (para os offsets, `MP:22`) e o scanner
pula espaços iniciais (`MS:26-30`). Regexes (`MS:7-14`):
`_findBeforeAssignment = :(\s*)`, `_findEndExpression = ;\s*`, `_findExpression = [^;]+`,
`_findImplicitBind = [^\s]+`, `_findLetAssignmentBefore = \s*=\s*`,
`_findLetIdentifier = [^\s=;]+`, `_findStartExpression = [^\s:;]+`, `_findWhitespace = \s+`.

Transições (cada falha → `invalidMicroExpression` cobrindo a expressão inteira):

| estado | casa | token → próximo estado |
|---|---|---|
| `initial` (`MS:140-157`) | `[^\s:;]+` = `let` | `letKeyword` → `afterLetKeyword` |
| | `[^\s:;]+` seguido **imediatamente** de `:` | `bindIdentifier` → `afterBindIdentifier` |
| | `[^\s:;]+` (outro) | `bindExpression` → `endExpression` |
| `afterLetKeyword` | `\s+` | `letKeywordAfter` → `letIdentifier` |
| `letIdentifier` (`MS:168-179`) | `[^\s=;]+` | `letIdentifier` → `afterLetIdentifier` (ou fim) |
| `afterLetIdentifier` (`MS:74-89`) | `;\s*` | `endExpression` → `initial` |
| | `\s*=\s*` | `letAssignmentBefore` → `letAssignment` |
| | `\s+` | `endExpression` → `implicitBind` |
| `letAssignment` | `[^;]+` | `letAssignment` → `endExpression` |
| `implicitBind` (`MS:131-138`) | `[^\s]+` | `bindIdentifier` → `beforeBindExpression` |
| `beforeBindExpression` | `\s+` | `bindExpressionBefore` → `bindExpression` |
| `afterBindIdentifier` | `:(\s*)` | `bindExpressionBefore` → `bindExpression` |
| `bindExpression` | `[^;]+` | `bindExpression` → `endExpression` |
| `endExpression` | fim / `;\s*` | — / `endExpression` → `initial` |

Notas: o `;` é separador **cru** — não respeita aspas nem parênteses; a chave precisa estar
**colada** ao `:` (`trackBy : f` é erro); `bindExpression` inclui os espaços finais antes do
`;` (não é aparado).

### R4.5 — Parser da microssintaxe (`MP:54-151`)
- `letKeyword` → `_parseLet`: `let x` (seguido de `;`, espaço ou fim) → `LetBindingAst(x)`
  **sem valor**; `let x = y` → `LetBindingAst(x, y.trimRight())`.
- `bindIdentifier` → `_parseBind`: `PropertyAst('$dir' + Maiúscula(nome[0]) + nome[1..],
  expr)` — `trackBy: f` em `ngFor` → `ngForTrackBy`; `of xs` (depois de `let x `) →
  `ngForOf`.
- `bindExpression` **só no primeiro token** → implícita: `PropertyAst(dir, expr)`.
- Qualquer outro token fora de `endExpression` → erro.

Exemplos:
- `let x of xs; trackBy: f; let i = index` com `dir = ngFor` → lets `[x, i=index]`, props
  `[ngForOf=xs, ngForTrackBy=f]`.
- `c; else: t` com `ngIf` → props `[ngIf=c, ngIfElse=t]`.
- `c; else t` → **erro** (`else` não colado a `:` vira `bindExpression` fora da 1ª posição).
- `let x of xs; index as i` → **erro**.
- `let x of xs; trackBy: f(';')` → **erro** (o `;` do literal separa).

**Porte:** `M:analisar` (`micro.rs:35-88`) com `M:partes` (`micro.rs:92-122`) **respeita
aspas e parênteses** e **ignora** partes que não entende (`index as i`, `else t`) em vez de
acusar erro; aceita `chave : expr`; representa `let x` como `(x, "$implicit")`. Nos casos
válidos o resultado bate. — **PARCIAL** (aceita o que o oficial rejeita).

---

## 5. Pós-parse no ngcompiler (antes de casar diretivas)

### R5.1 — Namespaces implícitos (`_NamespaceVisitor`, `ATP:1331-1376`; `TP:186-200`; `HT`)
- Prefixo do elemento: `ns:` explícito no nome, senão `getHtmlTagDefinition(nome)
  .implicitNamespacePrefix` (`svg` → `svg`, `math` → `math`, `HT:30-31`), senão o do pai.
  O nome vira `mergeNsAndName(prefixo, nomeLocal)` = `@prefixo:local` (se ambos não vazios).
  Ex.: `<svg><circle>` → `@svg:svg`, `@svg:circle`; `<svg:rect>` → `@svg:rect`.
- Atributo com `:` no nome: `xlink:href` → `@xlink:href`.
- No casamento de seletores o prefixo é removido (`_splitNsName`, `TP:139-143`).

**Porte:** não há tratamento de namespace (`grep` por `svg`/`namespace` em `V` só acha
recusas de `[attr.x:y]`). — **FALTA** (SVG gera outro código no oficial).

### R5.2 — Elementos filtrados (`_ElementFilter`, `ATP:1192-1235`)
São **removidos** do template, com *warning* (não erro): `<script>` e `<style>` (nome em
minúsculas) e `<link>` (nome exato) cujo `href` (atributo comparado em minúsculas) é
resolúvel por `isStyleUrlResolvable` (não vazio, não começa com `/`, sem esquema ou com
esquema `package`/`asset`).

**Porte:** não encontrado em `H` nem `V`. — **FALTA**.

### R5.3 — Validação do template cru (`_TemplateValidator`, `ATP:1378-1514`)
Em `ElementAst` e `EmbeddedTemplateAst`:
- atributos com o mesmo nome → *"Found multiple attributes with the same name: X."*;
- propriedades com o mesmo `_getPropertyName` (nome completo com `.postfix.unit`) →
  *"Found multiple properties with the same name: X."*;
- eventos com o mesmo nome completo (`name.reductions`) → *"Found multiple events with the
  same name: X. You should merge the handlers into a single statement."*
- evento com `:` no nome → *'":" is not allowed in event names: X'*; evento sem valor ou
  vazio → *"events must have a bound expresssion: X"* (sic).
- `#ref` com `-` → *'"-" is not allowed in reference names'*.
- Anotações: só `i18n…` (regex `i18n(?:\.(?:(locale)|(meaning)|(skip)))?(?::(.*)|$)`),
  `preserveWhitespace`, `skipOnPushValidation`, `skipSchemaValidationFor`; senão
  *"Invalid annotation"*. `@i18n`/`@i18n:x` sem valor, `@i18n.locale…`/`@i18n.meaning…`
  vazios e `@skipSchemaValidationFor` vazio têm mensagens próprias (`ATP:1397-1429`).

Como o desaçúcar já rodou, `[(x)]` duplicado com `[x]` também dispara a mensagem de
propriedades duplicadas.

**Porte:** só eventos duplicados (`V:eventos`, recusa). — **PARCIAL**.

---

## 6. Espaços em branco

### R6.1 — Escolha do modo (`ATP:155-164`)
`preserveWhitespace` do `@Component` (padrão `false`):
- `false` → `MinimizeWhitespaceVisitor().visitAllRoot(nós)` (R6.2–R6.5);
- `true` → `_PreserveWhitespaceVisitor` (`ATP:1610-1657`): só troca `U+E500` (`&ngsp;`)
  por espaço em todo texto, recursivamente em elementos, containers e templates.

**Porte:** sempre minimiza (`html.rs:157`); `preserveWhitespace:` não está em
`C:ARGUMENTOS_CONHECIDOS`, então o componente que o declara é **recusado** (`componente.rs`,
`fora.push(recusa(… NaoEntendido …))`). — **RECUSA** para `true`/`false` explícitos;
**COBERTO** no padrão.

### R6.2 — Onde se aplica (`WS:36-100`)
A redução é feita na lista de raízes e na lista de filhos de todo `ElementAst`,
`ContainerAst` e `EmbeddedTemplateAst` (com a árvore **já desaçucarada**), de cima para
baixo: primeiro a lista do pai é reduzida (olhando os vizinhos **crus**), depois cada filho é
visitado. `EmbeddedContentAst` não tem filhos.

**Exceção (`_bailOutToPreserveWhitespace`, `WS:12-31`)**: o nó é devolvido **intocado,
com toda a subárvore** — sem redução e **sem** a troca de `U+E500` — se:
- é `ElementAst` com `name == 'pre'` (exato);
- ou (`ElementAst`, `ContainerAst`, `EmbeddedTemplateAst`) tem a anotação
  `@preserveWhitespace`.

O `<template>` sintético de um `*` não tem anotações; quem decide é o elemento de dentro.

**Porte:** `H:minimizar_espacos` (`html.rs:626-641`), mesma ordem; bail-out por `nome ==
"pre"` ou anotação. — **COBERTO**.

### R6.3 — Remoção de texto só de espaços (`WS:137-189`)
Percorre os filhos `i = 0..n`. `prev` = o nó **já processado** anterior (fica `null` se o
texto anterior foi removido); `next` = o filho **cru** `i+1`. Um `TextAst` é **removido** se:
`collapse(prev, lastNode: true) && collapse(next, lastNode: false) && value.trim().isEmpty
&& !value.contains('\u00A0')`.

`_shouldCollapseAdjacentTo(nó)` (`WS:234-246`) é verdadeiro se:
1. `nó` é `null` (início/fim da lista, ou texto anterior removido) — **o `is!
   StandaloneTemplateAst` só é verdadeiro para `null`**: texto, interpolação, comentário e
   `<ng-content>` são `StandaloneTemplateAst` e **não** colapsam;
2. `ElementAst` cujo nome em minúsculas **não** está na lista em-linha:
   `a, abbr, acronym, b, bdo, big, br, button, cite, code, dfn, em, i, img, input, kbd,
   label, map, object, q, samp, script, select, small, span, strong, sub, sup, textarea,
   time, tt, var` (`WS:192-225`);
3. `ContainerAst`/`EmbeddedTemplateAst` pela regra de invólucro (`WS:255-295`): sem filhos
   → falso; olha o **último** filho se `lastNode` (vizinho à esquerda) ou o **primeiro**; se
   esse filho é texto só de espaços: com um filho só → **falso**; senão pula para o
   penúltimo/segundo. Decide `_shouldCollapseAdjacentTo(filho)` **com `lastNode` falso**.

### R6.4 — Colapso do texto restante (`WS:111-131`)
1. `U+00A0` → `U+E501` (protege do `\s` e do `trim`);
2. `RegExp(r'\s\s+')` → `' '` — **só sequências de 2+**: um `\n` sozinho fica;
3. `trimLeft()` se `collapse(prev, lastNode: true)`; `trimRight()` se `collapse(next,
   lastNode: false)`;
4. `U+E501` → `U+00A0`; vazio → nó removido (`prev` fica `null`);
5. depois, na visita do texto (`WS:103-108`), `U+E500` → `' '`.

### R6.5 — Exemplos (derivado)
| entrada | resultado |
|---|---|
| `<div>\n  <span>a</span>\n</div>` | `<div><span>a</span></div>` (1º texto: `prev=null` apara à esquerda → vazio; último: `next=null` apara à direita → vazio) |
| `<div>a</div>\n<div>b</div>` | texto removido (dois blocos) |
| `<b>x</b>\n\n<i>y</i>` | texto `' '` (vizinhos em linha) |
| `<p>a\nb</p>` | `a\nb` (um `\n` só não colapsa) |
| `<p>a  \n b</p>` | `a b` |
| `<div>\n  &ngsp;<b>x</b></div>` | `' '` (colapsa `"\n  "`→`" "`, apara à esquerda, `U+E500`→`' '`) |
| `<div>&nbsp;</div>` | `U+00A0` preservado (não removido, não aparado) |
| `{{a}} <!-- c --> {{b}}` | espaços mantidos (interpolação e comentário seguram) |
| `<div>\n<p *ngIf="c">x</p>\n</div>` | textos removidos (o `<template>` sintético olha o `<p>`) |
| `<pre>  a &ngsp; </pre>` | intocado, **com `U+E500` no texto** |

### R6.6 — Diferenças de classe de caractere
`\s` do Dart = conjunto do JavaScript (inclui `U+FEFF`, exclui `U+0085`); `trim` do Dart =
Unicode `White_Space` **mais** `U+FEFF` (inclui `U+0085`). `char::is_whitespace`/`str::trim`
do Rust = `White_Space` (inclui `U+0085`, exclui `U+FEFF`).

**Porte:** `H:reduzir_espacos`, `H:colapsar`, `H:colapsa_ao_lado`, `H:colapsa_envolvido`
(`html.rs:643-763`) seguem R6.3–R6.4 (inclusive `prev` processado/`next` cru, e a troca de
`U+E500` após aparar). **Defeito:** `H:colapsar` apara com `trim_start`/`trim_end`, que
**removem `U+00A0`** (é `White_Space`); o oficial o protege como `U+E501`. `<div>&nbsp;</div>`
some no porte; `&nbsp;x` ao lado de bloco perde o `&nbsp;`. Também usa
`c.is_whitespace()` em vez do `\s` do Dart (R6.6). `<template>` é reconhecido com `==
"template"` (oficial: `toLowerCase`). — **PARCIAL**.

### R6.7 — Compressão nas interpolações (visão, não parser) (`ECV:150-166`, `179-218`)
Com `preserveWhitespace` **falso**, as partes literais de uma interpolação que contêm `\n`
(e não contêm `U+00A0` nem `U+E500`) perdem todos os `\n` e são aparadas: a primeira à
esquerda, a última à direita. Em todos os casos `U+E500` → `' '`.

**Porte:** `V:comprimir_antes`/`V:comprimir_depois`. — **COBERTO**.

---

## 7. Do ngast ao `ng.*`: atributos, propriedades, eventos

### R7.1 — `ElementAst` (`ATP:260-301`)
`ng.ElementAst(name, attrs, inputs, outputs, references, directives, …)` com, **nesta ordem
de avaliação** (argumentos do Dart, esquerda para a direita — o comentário em `ATP:278-280`
depende disso):
1. `attrs` = `visitAttribute` de cada atributo (R7.2);
2. `inputs` = `_visitProperties` (R7.3);
3. `outputs` = `_visitEvents` (R7.5);
4. `references` (R9.1);
5. filhos, `ngContentIndex` (R8.6).

Anotações lidas aqui: `@skipOnPushValidation` (valida que o componente é OnPush, que o
elemento tem componente e que ele é `checkAlways`, `ATP:304-328`) e
`@skipSchemaValidationFor="seletor"` (`ATP:272-274`).

### R7.2 — Atributo literal → `AttrAst` (`ATP:552-571`, `870-885`)
- Atributo **com** `{{ }}` → **não** vira `AttrAst` (retorna `null`); é tratado em R7.3.
- Senão: `bindLiteralToDirective` — liga o valor como `LiteralPrimitive(value)` (ou
  `EmptyExpr` se `value == null`) a **toda** diretiva do nó com entrada de mesmo nome de
  template — **e o atributo continua** como `ng.AttrAst(name, valor)`, com
  `LiteralAttributeValue(value ?? '')` ou `I18nAttributeValue` se há `@i18n:name`.
- Ex.: `<input dir type="text" foo="1">` com `@Input('foo')` em `dir` → `AttrAst(type)`,
  `AttrAst(foo)` **e** a entrada `foo = '1'` da diretiva.

**Porte:** atributos em `No::Elemento.atributos`; ligação de atributo literal a entrada de
diretiva em `V` (casos do corpus). — **PARCIAL**.

### R7.3 — Propriedades (`_visitProperties`, `ATP:330-377`, `573-623`)
Primeiro as `[x]` (em ordem), depois os atributos com `{{ }}` (em ordem de atributo).
Para `[x]="e"`:
1. `parseBinding(value ?? '')`;
2. `bindPropertyToDirective(_getPropertyName(ast))` — se alguma diretiva tem entrada com esse
   nome de template, vira `BoundDirectivePropertyAst` (substituindo uma anterior de mesmo
   nome, `_removeExisting`) e **sai** da lista do elemento;
3. em `<template>`, o que sobrou é erro *"Can't bind to 'x' since it isn't an input of any
   bound directive. …"* (+ dica para `ngForIn`: *"did you mean to write 'of' instead of
   'in'?"*);
4. senão `createElementPropertyAst(elementName, _getPropertyName(ast), …)` (R7.4).

Para `nome="texto {{e}}"`: `parseInterpolation(value)`; `bindInterpolationToDirective(nome)`
(nome do atributo como está); senão `createElementPropertyAst(elementName, nome, …)` — o
nome passa pelas mesmas regras de `[x]`: `attr.x="{{e}}"`, `class.x="{{e}}"`,
`style.x="{{e}}"` são as ligações de atributo, classe e estilo.

Ordem de precedência numa entrada de diretiva (último vence, via `_removeExisting`):
atributo literal → `[x]` → atributo interpolado.

`_getPropertyName` (`ATP:1178-1186`) remonta `name[.postfix[.unit]]`.

**Porte:** `V:acao`, `V:atributo_interpolado`. — **PARCIAL** (ver R7.4 e R10.4).

### R7.4 — `createElementPropertyAst` (`TP:39-130`)
Divide o nome por `.`:

| forma | `boundPropertyName` | tipo | security context | validação |
|---|---|---|---|---|
| `x` (1 parte) | `getMappedPropName(x)` | `property` | `securityContext(tag, mapeado)` | `hasProperty(tag, mapeado)` falso → *"Can't bind to 'X' since it isn't a known native property or known directive. Please fix typo or add to directives list."*; se `X == 'ngclass'` → *"Please use camel-case ngClass instead of ngclass in your template"* |
| `attr.x[.u]` | `x` (sem namespace; `ns:x` → `namespace=ns`, nome `x`) | `attribute` | `securityContext(tag, getMappedPropName(x))` | `x` começando por `on` (sem distinguir maiúsculas) → *"Binding to event attribute 'x' is disallowed for security reasons, please use (…)=..."*; `u` diferente de `if` → *'Invalid attribute unit "u"'* |
| `class.x` | `x` | `cssClass` | `none` | — |
| `style.x[.u]` | `x`, `unit = u` | `style` | `style` | — |
| outro prefixo | — | `null` | `null` | *"Invalid property name 'N'"* |

`hasProperty` só é consultado na forma de 1 parte. Exemplos:
- `<div [title]="t">` → property `title`, context `none`.
- `<div [class]="c">` → property `className` (mapeado), `hasProperty('div','className')` ok.
- `<label [for]="x">` → property `for` (**não** é mapeado), `hasProperty('label','for')`
  **falso** → erro. (Use `[htmlFor]` ou `[attr.for]`.)
- `<input [readonly]="r">` → `readOnly`; `<div [tabindex]="1">` → `tabIndex`.
- `<a [href]="u">` → context `url`; `<a [attr.href]="u">` → context `url` também.
- `<div [innerHtml]="h">` → `innerHTML`, context `html`.

**Porte:** `V:acao` trata `class`/`className`/`attr.class`, `class.x`, `attr.x`
(recusa `attr.x.if` e namespace), `style.x[.u]` (recusa 3 partes), e propriedade simples
com `innerHtml`→`innerHTML`; **recusa** `readonly`/`tabindex`/`tabIndex` como propriedade;
aplica `V:saneador` com a tag **em minúsculas** (oficial: tag como está, então `<A
[href]>` não é saneado no oficial). **Não** valida `hasProperty`: `[foo]` em `<div>` gera
`setProperty` no porte e é erro no oficial. Não acusa `attr.onclick`. — **PARCIAL**.

### R7.5 — Eventos (`ATP:388-396`, `492-550`)
`outputs` = eventos do template (em ordem) **seguidos** dos `@HostListener` das diretivas do
nó que **não** são componente (`_collectHostListeners`, `ATP:1072-1098`). Cada um:
`parseAction(value)`; se alguma diretiva do nó tem `@Output` com esse nome (o `name` sem
reduções, `ATP:498`), vira `BoundDirectiveEventAst` em **todas** as que casam e sai da lista
(`bindEventToDirective`, `ATP:922-942`); senão `BoundEventAst(_getEventName(ast))` (nome
completo com reduções). Em `<template>`, os eventos que sobram são **descartados** sem erro
(`ATP:421-427`).

**Porte:** `V:eventos` e o tratamento de saídas de diretiva em `V`. — **PARCIAL** (evento
em `<template>` sem diretiva é recusado no porte, descartado no oficial).

### R7.6 — `<template>`, `<ng-container>`, `<ng-content>`, texto
- `EmbeddedTemplateAst` (`ATP:417-440`): propriedades/eventos só servem às diretivas;
  `attrs` = atributos literais; `variables` = `let-`; descritor de casamento com nome
  `template` (R8.1).
- `NgContainerAst` (`ATP:406-414`): só filhos; o contexto herda o matcher de `ng-content` do
  pai.
- `NgContentAst(índice = contador global do template em ordem de visita,
  ngContentIndex, ref)` (`ATP:471-479`).
- `TextAst(value, ngContentIndex('*'))`, `BoundTextAst(parseInterpolation('{{v}}'))`;
  comentários somem (`ATP:638-673`).

---

## 8. Seletores e casamento de diretivas

### R8.1 — Descrição do nó (`_selector`, `ATP:1147-1166`; `createElementCssSelector`, `TP:136-159`)
Um `CssSelector` com:
- `element` = nome sem namespace (`@svg:rect` → `rect`); em `EmbeddedTemplateAst`,
  `template`;
- atributos, em ordem: **atributos** `[nome, value]`, depois **propriedades**
  `[_getPropertyName, value]` (o valor é a **expressão**), depois **eventos**
  `[_getEventName, value]`. `let-` e `#ref` **não** entram. Cada um vira
  `ExactAttributeMatcher(nomeSemNs, value?.toLowerCase())` — valor `null` fica `null`;
- se o nome (em minúsculas) é `class` e o valor não é nulo: `jsSplit(valor.trim(), \s+)`, cada
  classe em minúsculas (`addClassName`).

Ex.: `<input type="Text" [(ngModel)]="m" class="A b">` → `input[type=text][class=a b]
[ngModel=m][ngModelChange=m = $event].a.b`.

**Porte:** `S:Elemento::do_template` (`seletor.rs:355-373`) e `S:Elemento::atributo`
(`seletor.rs:340-349`): mesma ordem (atributos, propriedades+bananas, eventos,
`xChange`). Diferenças: atributo sem valor vira `Some("")` (oficial `null` — só importa para
`[x=""]`, que no oficial **não** casa atributo sem valor); os `let-x` de `<template>` entram
como atributos. — **PARCIAL**.

### R8.2 — `CssSelector.parse` (`SEL:4-77`)
Regex global aplicada com `allMatches` (o que não casa é pulado):
`(:not\()` | `([-\w]+)` (tag) | `(?:\.([-\w]+))` (classe) |
`(?:\[([-\w]+)(?:([~|^$*]?=)(['"]?)([^\]'"]*)\6)?\])` (atributo) | `(\))` | `(\s*,\s*)`.
- `:not(` abre um seletor negado (aninhar → `StateError`); `,` dentro de `:not` →
  `StateError`; `)` fecha.
- `,` fecha o seletor atual e começa outro (lista).
- Ao fechar, um seletor só com `:not(...)` ganha `element = '*'`.
- `addAttribute(nome, matcher, valor)` (`SEL:111-138`): valor em minúsculas; sem operador →
  `SetAttributeMatcher`; `=` → `ExactAttributeMatcher`; `~= |= ^= $= *=` só são
  registrados se o valor não for vazio. `addClassName` põe em minúsculas. **Nomes de
  tag e de atributo não mudam de caixa.**

Ex.: `form:not([ngNoForm]),[ngForm]` → `[form:not([ngNoForm]) , [ngForm]]`;
`input[type=checkbox][ngModel]`.

**Porte:** `S:Seletor::analisar` (`seletor.rs:166-255`), mesma ordem de alternativas.
Não lança nos casos de `StateError`. — **COBERTO** (salvo os erros).

### R8.3 — Matchers de atributo (`AM:8-112`)
| op | casa se |
|---|---|
| `[a]` | sempre |
| `[a=v]` | `valor == v` (inclui `null == null`) |
| `[a~=v]` | `valor!.split(\s+).contains(v)` |
| `[a\|=v]` | `valor == v \|\| valor!.startsWith('v-')` |
| `[a^=v]` / `[a$=v]` / `[a*=v]` | `startsWith` / `endsWith` / `contains` (com `valor!`) |

**Porte:** `S:Casamento::aceita` (`seletor.rs:52-64`); nos que usam `!` no oficial, valor
nulo não casa (no oficial seria exceção). — **COBERTO**.

### R8.4 — `SelectorMatcher` (`SEL:168-342`) e `SelectorContext.finalize` (`SEL:351-378`)
Árvore: elemento (terminal se sem classes e atributos; senão parcial), depois cada classe
(terminal só a última, se sem atributos), depois cada atributo (o último é terminal). Um
seletor casa quando **o elemento bate, todas as classes estão no nó e todos os atributos
casam**, e nenhum dos `:not(...)` casa (`finalize` usa um `SelectorMatcher` à parte só com os
negados). Numa lista (`a, b`), `SelectorListContext.alreadyMatched` impede o callback de
disparar duas vezes no mesmo `match`.
- `*` terminal (ex.: seletor só `:not(x)`) casa qualquer elemento; `*` com classe/atributo
  nunca casa (a parcial é buscada pelo nome do nó).
- Seletor vazio (sem elemento, classe ou atributo) nunca casa.
- **Defeito do oficial:** `_matchTerminal` faz `List.from(selectables!)` quando há terminal
  `'*'` (`SEL:310-314`); se o nome do nó não tem terminal próprio, `selectables` é `null` e o
  `!` lança. Isto é: com alguma diretiva de seletor `*`/`:not(...)` sozinho, casar um nó
  cuja tag não tem seletor terminal quebra a compilação.

**Porte:** `S:Seletor::casa`/`casa_como_nao` (`seletor.rs:278-318`), por seletor, sem
árvore. — **COBERTO** nas regras de casamento; o defeito do `*` não é reproduzido.

### R8.5 — Ordem das diretivas num nó (`ATP:1003-1064`; `PP:136-147`)
1. `directives` do componente passa por `removeDuplicates` (mesmo `type.name` e
   `moduleUrl`, primeiro vence, `TP:161-184`).
2. `_matchDirectives`: casa todos os seletores e devolve `directives.where(casadas.contains)`
   — **a ordem da lista `directives:`**, não a do HTML nem a do seletor.
3. `_filterExcessComponents`: mantém só o **primeiro componente**; diretivas todas.
4. Depois do provider parser, `transformedDirectiveAsts` reordena pelas dependências de
   injeção (`PP:136-147`; ver a seção de injeção).
5. As entradas de cada diretiva são reordenadas pela ordem de declaração de `inputs`
   (`_SortInputsVisitor`, `ATP:1659-1676`).

**Porte:** `V:diretivas_casadas` percorre `usadas` na ordem de `directives:` (expandida e sem
repetição em `lib.rs`, `vistos`); componente casado por seletor composto é recusado em
`V:guarda_do_elemento`. — **COBERTO/RECUSA**.

### R8.6 — `ngContentIndex` (`ATP:975-986`, `1100-1120`, `398-490`)
O matcher do elemento **pai** que tem componente: um `SelectorMatcher<int>` com cada
`ngContentSelectors[i] != '*'`; o índice do `*` é o curinga. Para um nó: casa a descrição
(R8.1), junta os índices, ordena, pega o **menor**; nada casou → curinga (ou `null`). Cada
seletor casado é anotado em `matchedNgContentSelectors`.
- Texto e interpolação usam a descrição `*`.
- `<template>` sintético de `*dir` (origem `StarAst`, ou template cuja origem é template
  sintético) com filho único `ElementAst`: usa a descrição **do filho** (`ATP:442-468`).
- `<ng-content>` projetado: usa `CssSelector.parse(ngProjectAs)[0]` se houver, senão
  `ng-content[select=<selector>]` (`ATP:485-490`).
- `<ng-container>` herda o matcher do pai.

**Porte:** projeção em `V` (fora do escopo deste arquivo); `ngProjectAs` não existe no AST do
porte (R3.5). — **PARCIAL**.

---

## 9. Referências e variáveis `let-`

### R9.1 — `#ref` (`ATP:630-636`, `854-868`)
`ng.ReferenceAst(variable, identifierForReference(identifier))`:
1. percorre as diretivas **do nó**, em ordem: a **primeira** que é componente (quando `#ref`
   sem valor) ou cujo `exportAs == valor` → o token do tipo dela;
2. senão, em `<template>` → `TemplateRef`;
3. senão, em `<ng-content>` com referência → `ngContentRef`;
4. senão `null` — que a visão resolve como **o próprio elemento** (`CE:228-233` em
   `view_compiler/compile_element.dart`). Logo `#f="naoExiste"` **não** é erro: vira o
   elemento.

Ex.: `<form #f="ngForm">` com `NgForm(exportAs: 'ngForm')` → instância de `NgForm`;
`<my-cmp #c>` → instância do componente; `<div #d>` → o `HtmlElement`; `<template #t>` →
`TemplateRef`.

**Porte:** `V` (`exportada`, dentro do tratamento do elemento): aceita `#r="x"` só com
**exatamente uma** diretiva do nó exportando `x` (duas → recusa; oficial pega a primeira);
sem diretiva → recusa (oficial: o elemento). — **PARCIAL/RECUSA**.

### R9.2 — `let-x="k"` (`ATP:625-628`)
Só em `<template>` (erro do parser nos outros). `ng.VariableAst(name, value)`; `value ==
null` significa o `$implicit` do contexto. Os `let` da microssintaxe (R4.5) chegam pelo
mesmo caminho.

**Porte:** `M:Micro.locais` (`(nome, "$implicit" | chave)`) e `let-x` como atributo de
`<template>` escrito (`V:molde_com_diretiva`). — **COBERTO** nos casos aceitos.

---

## 10. Esquema DOM (`DomElementSchemaRegistry`, `DSR`)

### R10.1 — Montagem (`DSR:214-255`)
Cada linha de `_schema` é `tags^pai|props`. `('${parte0}^').split('^')` dá o nome e o pai
(`''` quando não há `^`). Todas as tags da lista (`h1,h2,…`) compartilham o **mesmo** mapa.
O mapa começa com uma cópia do pai (propriedades, atributos e eventos). Prefixo da
propriedade: `*` = evento (guardado em minúsculas, **não** vira propriedade), `!` boolean,
`#` number, `%` object, sem prefixo string. O conjunto de **atributos** recebe
`_toAttribute(prop) = (_propToAttrMap[prop] ?? prop).toLowerCase()`.
- `''` (a linha `^*|…`) herda de `*`; tag sem `^` herda de `''`.

### R10.2 — Consultas (`DSR:257-278`)
- `hasProperty(tag, prop)`: `schema[tag.toLowerCase()] ?? schema['unknown']`, e
  `prop` **sensível a maiúsculas**. Obs.: chaves como `@svg:clipPath` têm maiúsculas; a busca
  em minúsculas não as acha e cai em `unknown`.
- `hasAttribute(tag, attr)`: `attr.toLowerCase()` no conjunto de atributos.
- `hasEvent(tag, ev)`: `ev.toLowerCase()` no conjunto de eventos.
- `getMappedPropName(p) = _attrToPropMap[p] ?? p`.
- `securityContext(tag, prop)`: `'$tag|$prop'`, senão `'*|$prop'`, senão `none` — **tag como
  veio** (sem minúsculas).

### R10.3 — Tabela completa atributo → propriedade (`_attrToPropMap`, `DSR:198-203`)

| nome escrito | propriedade |
|---|---|
| `class` | `className` |
| `innerHtml` | `innerHTML` |
| `readonly` | `readOnly` |
| `tabindex` | `tabIndex` |

Não há outras. Em particular **`for` → `htmlFor` não existe**; `htmlFor → for` e
`className → class` estão só em `_propToAttrMap` (`DSR:205-208`), usado para derivar os
nomes de **atributo** do esquema (`hasAttribute`). Chaves sensíveis a maiúsculas:
`Readonly` não é mapeado.

Derivação propriedade → atributo (`_toAttribute`), as 74 propriedades cujo nome de atributo
difere (gerado dos dados de `DSR:49-195`):
`aLink→alink, acceptCharset→acceptcharset, accessKey→accesskey,
allowFullscreen→allowfullscreen, bgColor→bgcolor, cellPadding→cellpadding,
cellSpacing→cellspacing, chOff→choff, classList→classlist, className→class,
codeBase→codebase, codeType→codetype, colSpan→colspan, contentEditable→contenteditable,
controlsList→controlslist, crossOrigin→crossorigin, currentScale→currentscale,
currentTime→currenttime, dateTime→datetime, defaultChecked→defaultchecked,
defaultMuted→defaultmuted, defaultPlaybackRate→defaultplaybackrate,
defaultSelected→defaultselected, defaultValue→defaultvalue, dirName→dirname,
disablePictureInPicture→disablepictureinpicture,
disableRemotePlayback→disableremoteplayback, formAction→formaction,
formEnctype→formenctype, formMethod→formmethod, formNoValidate→formnovalidate,
formTarget→formtarget, frameBorder→frameborder, htmlFor→for, httpEquiv→httpequiv,
innerHTML→innerhtml, innerText→innertext, isMap→ismap, longDesc→longdesc,
marginHeight→marginheight, marginWidth→marginwidth, maxLength→maxlength,
minLength→minlength, noHref→nohref, noResize→noresize, noShade→noshade,
noValidate→novalidate, noWrap→nowrap, outerHTML→outerhtml, outerText→outertext,
playbackRate→playbackrate, readOnly→readonly, relList→rellist, returnValue→returnvalue,
rowSpan→rowspan, scrollAmount→scrollamount, scrollDelay→scrolldelay,
scrollLeft→scrollleft, scrollTop→scrolltop, selectedIndex→selectedindex,
selectionDirection→selectiondirection, selectionEnd→selectionend,
selectionStart→selectionstart, tFoot→tfoot, tHead→thead, tabIndex→tabindex,
trueSpeed→truespeed, useMap→usemap, vAlign→valign, vLink→vlink,
valueAsDate→valueasdate, valueAsNumber→valueasnumber, valueType→valuetype,
zoomAndPan→zoomandpan`.

**Porte:** `V:propriedade_mapeada` (as 4 entradas, usada para o contexto de segurança de
`attr.x`); em `V:acao` só `innerHtml` é mapeado e `readonly`/`tabindex` são recusados. O
conjunto de atributos não existe. — **PARCIAL**.

### R10.4 — Tabela completa de security contexts (`DSR:290-331`)
Inicialização preguiçosa num mapa estático. Chave `tag|prop`; fallback `*|prop`; senão
`none`.

| contexto | chaves |
|---|---|
| `html` | `iframe\|srcdoc`, `*\|innerHTML`, `*\|outerHTML` |
| `style` | `*\|style` |
| `url` | `*\|formAction`, `area\|href`, `area\|ping`, `audio\|src`, `a\|href`, `a\|ping`, `blockquote\|cite`, `body\|background`, `del\|cite`, `form\|action`, `img\|src`, `img\|srcset`, `input\|src`, `ins\|cite`, `q\|cite`, `source\|src`, `source\|srcset`, `video\|poster`, `video\|src` |
| `resourceUrl` | `applet\|code`, `applet\|codebase`, `base\|href`, `embed\|src`, `frame\|src`, `head\|profile`, `html\|manifest`, `iframe\|src`, `link\|href`, `media\|src`, `object\|codebase`, `object\|data`, `script\|src`, `track\|src` |

Consequências da regra:
- a chave usa o nome **mapeado**: `[innerHtml]` → `innerHTML` → `html`; `[attr.formaction]`
  → `getMappedPropName('formaction') = 'formaction'` → **`none`** (a chave é `formAction`);
- `class.x` → `none`, `style.x` → `style` (fixo em `TP:100-108`);
- `media|src` nunca casa um elemento real (a tag é `audio`/`video`), mas `audio|src` e
  `video|src` estão em `url`.

**Porte:** `V:saneador` tem exatamente estas quatro listas e a mesma ordem de busca
(`tag|prop`, depois `*|prop`), mas com a tag em minúsculas (R7.4). — **COBERTO** (salvo a
caixa da tag).

### R10.5 — `_schema` completo (`DSR:49-195`), verbatim
Necessário para `hasProperty`/`hasAttribute`/`hasEvent`:

```text
*|%classList,className,id,innerHTML,*beforecopy,*beforecut,*beforepaste,*copy,*cut,*paste,*search,*selectstart,*webkitfullscreenchange,*webkitfullscreenerror,*wheel,outerHTML,#scrollLeft,#scrollTop,role
^*|accessKey,autocapitalize,!autofocus,contentEditable,dir,!draggable,enterkeyhint,!hidden,innerText,inputmode,is,itemid,itemprop,itemref,!itemscope,itemtype,lang,nonce,*abort,*autocomplete,*autocompleteerror,*beforecopy,*beforecut,*beforepaste,*blur,*cancel,*canplay,*canplaythrough,*change,*click,*close,*contextmenu,*copy,*cuechange,*cut,*dblclick,*drag,*dragend,*dragenter,*dragleave,*dragover,*dragstart,*drop,*durationchange,*emptied,*ended,*error,*focus,*input,*invalid,*keydown,*keypress,*keyup,*load,*loadeddata,*loadedmetadata,*loadstart,*message,*mousedown,*mouseenter,*mouseleave,*mousemove,*mouseout,*mouseover,*mouseup,*mousewheel,*mozfullscreenchange,*mozfullscreenerror,*mozpointerlockchange,*mozpointerlockerror,*paste,*pause,*play,*playing,*progress,*ratechange,*reset,*resize,*scroll,*search,*seeked,*seeking,*select,*selectstart,*show,*stalled,*submit,*suspend,*timeupdate,*toggle,*volumechange,*waiting,*webglcontextcreationerror,*webglcontextlost,*webglcontextrestored,*webkitfullscreenchange,*webkitfullscreenerror,*wheel,outerText,!spellcheck,%style,#tabIndex,title,!translate
media|!autoplay,!controls,%controlsList,%crossOrigin,#currentTime,!defaultMuted,#defaultPlaybackRate,!disableRemotePlayback,!loop,!muted,*encrypted,#playbackRate,preload,src,#volume
@svg:^*|*abort,*autocomplete,*autocompleteerror,*blur,*cancel,*canplay,*canplaythrough,*change,*click,*close,*contextmenu,*cuechange,*dblclick,*drag,*dragend,*dragenter,*dragleave,*dragover,*dragstart,*drop,*durationchange,*emptied,*ended,*error,*focus,*input,*invalid,*keydown,*keypress,*keyup,*load,*loadeddata,*loadedmetadata,*loadstart,*mousedown,*mouseenter,*mouseleave,*mousemove,*mouseout,*mouseover,*mouseup,*mousewheel,*pause,*play,*playing,*progress,*ratechange,*reset,*resize,*scroll,*seeked,*seeking,*select,*show,*stalled,*submit,*suspend,*timeupdate,*toggle,*volumechange,*waiting,%style,#tabIndex
@svg:graphics^@svg:|
@svg:animation^@svg:|*begin,*end,*repeat
@svg:geometry^@svg:|
@svg:componentTransferFunction^@svg:|
@svg:gradient^@svg:|
@svg:textContent^@svg:graphics|
@svg:textPositioning^@svg:textContent|
a|charset,coords,download,hash,host,hostname,href,hreflang,name,password,pathname,ping,port,protocol,rel,rev,search,shape,target,text,type,username
area|alt,coords,hash,host,hostname,href,!noHref,password,pathname,ping,port,protocol,search,shape,target,username
audio^media|
br|clear
base|href,target
body|aLink,background,bgColor,link,*beforeunload,*blur,*error,*focus,*hashchange,*languagechange,*load,*message,*offline,*online,*pagehide,*pageshow,*popstate,*rejectionhandled,*resize,*scroll,*storage,*unhandledrejection,*unload,text,vLink
button|!disabled,formAction,formEnctype,formMethod,!formNoValidate,formTarget,name,type,value
canvas|#height,#width
content|select
dl|!compact
datalist|
details|!open
dialog|!open,returnValue
dir|!compact
div|align
embed|align,height,name,src,type,width
fieldset|!disabled,name
font|color,face,size
form|acceptCharset,action,autocomplete,encoding,enctype,method,name,!noValidate,target
frame|frameBorder,longDesc,marginHeight,marginWidth,name,!noResize,scrolling,src
frameset|cols,*beforeunload,*blur,*error,*focus,*hashchange,*languagechange,*load,*message,*offline,*online,*pagehide,*pageshow,*popstate,*rejectionhandled,*resize,*scroll,*storage,*unhandledrejection,*unload,rows
hr|align,color,!noShade,size,width
head|
h1,h2,h3,h4,h5,h6|align
html|version
iframe|align,allow,!allowFullscreen,frameBorder,height,longDesc,marginHeight,marginWidth,name,%sandbox,scrolling,src,srcdoc,width
img|align,alt,border,%crossOrigin,#height,#hspace,!isMap,longDesc,lowsrc,name,sizes,src,srcset,useMap,#vspace,#width
input|accept,align,alt,autocomplete,!checked,!defaultChecked,defaultValue,dirName,!disabled,%files,formAction,formEnctype,formMethod,!formNoValidate,formTarget,#height,!incremental,!indeterminate,max,#maxLength,min,#minLength,!multiple,name,pattern,placeholder,!readOnly,!required,selectionDirection,#selectionEnd,#selectionStart,#size,src,step,type,useMap,value,%valueAsDate,#valueAsNumber,#width
keygen|challenge,!disabled,keytype,name
li|type,#value
label|htmlFor
legend|align
link|as,charset,%crossOrigin,!disabled,href,hreflang,integrity,media,rel,%relList,rev,%sizes,target,type
map|name
marquee|behavior,bgColor,direction,height,#hspace,#loop,#scrollAmount,#scrollDelay,!trueSpeed,#vspace,width
menu|!compact
meta|content,httpEquiv,name,scheme
meter|#high,#low,#max,#min,#optimum,#value
ins,del|cite,dateTime
ol|!compact,!reversed,#start,type
object|align,archive,border,code,codeBase,codeType,data,!declare,height,#hspace,name,standby,type,useMap,#vspace,width
optgroup|!disabled,label
option|!defaultSelected,!disabled,label,!selected,text,value
output|defaultValue,%htmlFor,name,value
p|align
param|name,type,value,valueType
picture|
pre|#width
progress|#max,#value
q,blockquote,cite|
script|!async,charset,%crossOrigin,!defer,event,htmlFor,integrity,src,text,type
select|!disabled,#length,!multiple,name,!required,#selectedIndex,#size,value
shadow|
source|media,sizes,src,srcset,type
span|
style|!disabled,media,type
caption|align
th,td|abbr,align,axis,bgColor,ch,chOff,#colSpan,headers,height,!noWrap,#rowSpan,scope,vAlign,width
col,colgroup|align,ch,chOff,#span,vAlign,width
table|align,bgColor,border,%caption,cellPadding,cellSpacing,frame,rules,summary,%tFoot,%tHead,width
tr|align,bgColor,ch,chOff,vAlign
tfoot,thead,tbody|align,ch,chOff,vAlign
template|
textarea|#cols,defaultValue,dirName,!disabled,#maxLength,#minLength,name,placeholder,!readOnly,!required,#rows,selectionDirection,#selectionEnd,#selectionStart,value,wrap
title|text
track|!default,kind,label,src,srclang
ul|!compact,type
unknown|
video^media|!disablePictureInPicture,#height,poster,#width
@svg:a^@svg:graphics|
@svg:animate^@svg:animation|
@svg:animateMotion^@svg:animation|
@svg:animateTransform^@svg:animation|
@svg:circle^@svg:geometry|
@svg:clipPath^@svg:graphics|
@svg:cursor^@svg:|
@svg:defs^@svg:graphics|
@svg:desc^@svg:|
@svg:discard^@svg:|
@svg:ellipse^@svg:geometry|
@svg:feBlend^@svg:|
@svg:feColorMatrix^@svg:|
@svg:feComponentTransfer^@svg:|
@svg:feComposite^@svg:|
@svg:feConvolveMatrix^@svg:|
@svg:feDiffuseLighting^@svg:|
@svg:feDisplacementMap^@svg:|
@svg:feDistantLight^@svg:|
@svg:feDropShadow^@svg:|
@svg:feFlood^@svg:|
@svg:feFuncA^@svg:componentTransferFunction|
@svg:feFuncB^@svg:componentTransferFunction|
@svg:feFuncG^@svg:componentTransferFunction|
@svg:feFuncR^@svg:componentTransferFunction|
@svg:feGaussianBlur^@svg:|
@svg:feImage^@svg:|
@svg:feMerge^@svg:|
@svg:feMergeNode^@svg:|
@svg:feMorphology^@svg:|
@svg:feOffset^@svg:|
@svg:fePointLight^@svg:|
@svg:feSpecularLighting^@svg:|
@svg:feSpotLight^@svg:|
@svg:feTile^@svg:|
@svg:feTurbulence^@svg:|
@svg:filter^@svg:|
@svg:foreignObject^@svg:graphics|
@svg:g^@svg:graphics|
@svg:image^@svg:graphics|
@svg:line^@svg:geometry|
@svg:linearGradient^@svg:gradient|
@svg:mpath^@svg:|
@svg:marker^@svg:|
@svg:mask^@svg:|
@svg:metadata^@svg:|
@svg:path^@svg:geometry|
@svg:pattern^@svg:|
@svg:polygon^@svg:geometry|
@svg:polyline^@svg:geometry|
@svg:radialGradient^@svg:gradient|
@svg:rect^@svg:geometry|
@svg:svg^@svg:graphics|#currentScale,#zoomAndPan
@svg:script^@svg:|type
@svg:set^@svg:animation|
@svg:stop^@svg:|
@svg:style^@svg:|!disabled,media,title,type
@svg:switch^@svg:graphics|
@svg:symbol^@svg:|
@svg:tspan^@svg:textPositioning|
@svg:text^@svg:textPositioning|
@svg:textPath^@svg:textContent|
@svg:title^@svg:|
@svg:use^@svg:graphics|
@svg:view^@svg:|#zoomAndPan
```

Exemplos de `hasProperty`: `('div','title')` → herda de `''` → sim; `('my-cmp','title')` →
`unknown` (herda `''`) → sim; `('div','value')` → não → erro de R7.4;
`('@svg:circle','style')` → herda `@svg:` → sim; `('@svg:clipPath', …)` → procurado como
`@svg:clippath` → `unknown`.

**Porte:** não há tabela `_schema` em `D` nem em `V`. — **FALTA**.

---

## 11. `MissingDirectiveValidator` (ligado por padrão) (`MDV`)

`validateMissingDirectives` é `!hasPolicyExceptionInPackage('EXCLUDED_VALIDATE_MISSING_DIRECTIVES')`
(`ngcompiler/lib/v2/context.dart:335-337`): **ativo** fora das exceções de política. Roda
sobre a árvore `ng.*` já com provedores (`ATP:208-210`).

### R11.1 — Elemento (`MDV:40-86`)
Nome sem `@xhtml:`. Erro *"Can't find '<X>'. Please check that the spelling is correct, and
that the intended component is included in the host component's list of directives. See more
details go/skipschemavalidationfor.\n…"* **a não ser que**: algum seletor de
`@skipSchemaValidationFor` tenha esse elemento; ou seja tag HTML (`_htmlTagNames` de
`view_compiler_utils.dart`, `detectHtmlElementFromTagName`); ou algum seletor de diretiva do
nó tenha `element == nome`; ou um `matchedNgContentSelectors` tenha; ou esteja na allowlist
(`SSV:127-129`, seletores só de elemento como `aplos-chart`); ou comece com `@svg`. Seletor de
`@skipSchemaValidationFor` que não casa o elemento (descrito por nome + atributos + saídas)
também é erro.

### R11.2 — Atributo (`MDV:149-176`)
Em elemento não-`@svg`, `AttrAst` é erro *"Can't bind to 'X' since it isn't an input of any
bound directive or a native property. …"* a não ser que: casa um atributo de
`@skipSchemaValidationFor`; `hasAttribute(tag, X)` (R10.2); é entrada de diretiva do nó; é
atributo de algum seletor de diretiva do nó; de algum `matchedNgContentSelectors`; da
allowlist (`SSV:131-134`); `aria-*` conhecido (`aria_attributes.dart`); `attributeDeps` (os
`@Attribute('x')` injetados); ou `debugid`, `debug-id`, `debugId`, `data-test-id`.
Ex.: `<div data-x="1">` sem diretiva → **erro** (`data-x` não está no esquema).

### R11.3 — Evento (`MDV:213-237`)
`BoundEventAst` (os que sobraram do DOM): nome antes do 1º `.`; erro *"Can't bind to (X)
since it isn't an output of any bound directive or a native event. …"* a não ser que casa
`@skipSchemaValidationFor`, `isNativeHtmlEvent(x.toLowerCase())` (`html_events.dart`),
`hasEvent(tag, x)` ou está em `_customEvents` da allowlist.

**Porte:** há `D:tag_html`, `V:evento_nativo` e recusas pontuais (tag que não é HTML nem
componente, nomes de diretivas do ecossistema), mas não a validação de atributos/eventos pelo
esquema, nem `aria`, nem `@skipSchemaValidationFor`. — **FALTA** (o porte aceita
`<div data-x>` e `foo="1"` que o oficial rejeita).

---

## 12. Erros que o oficial acusa

### R12.1 — `ParserErrorCode` do ngast (`EX:3-56`), mensagem exata
| código | mensagem | onde |
|---|---|---|
| `cannotFindMatchingClose` | Cannot find matching close element to this | `RP:790,809` |
| `danglingCloseElement` | Closing tag is dangling and no matching open tag can be found | `RP:731,818` |
| `duplicateStarDirective` | Already found a *-directive, limit 1 per element. | `RP:378,766` |
| `duplicateSelectDecorator` | Only 1 'select' decorator can exist in <ng-content>, found duplicate | `RP:493` |
| `duplicateProjectAsDecorator` | Only 1 'ngProjectAs' decorator can exist in <ng-content>, found duplicate | `RP:505` |
| `duplicateReferenceDecorator` | Only 1 reference decorator can exist in <ng-content>, found duplicate | `RP:517` |
| `elementDecorator` | Expected element decorator after whitespace | `SC:576,897` |
| `elementDecoratorAfterPrefix` | Expected element decorator identifier after prefix | `SC:930,963,996,1029`, `RP:245,267,361` |
| `elementDecoratorSuffixBeforePrefix` | Found special decorator suffix before prefix | `SC:586` |
| `elementDecoratorValue` | Expected quoted value following '=' | `SC:681` |
| `elementDecoratorValueMissingQuotes` | Decorator values must contain quotes | `SC:658` |
| `elementIdentifier` | Expected element tag name | `SC:723,741` |
| `expectedAfterElementIdentifier` | Expected either whitespace or close tag end after element identifier | `SC:331,396` |
| `expectedEqualSign` | Expected '=' between decorator and value | `SC:242` |
| `expectedStandalone` | Expected standalone token | `RP:752` |
| `expectedTagClose` | Expected tag close. | `SC:232,298,774` |
| `expectedToken` | Unexpected token | vários |
| `expectedWhitespaceBeforeNewDecorator` | Expected whitespace before a new decorator | `SC:220,287,384` |
| `emptyInterpolation` | Interpolation expression cannot be empty | `SC:862` |
| `invalidDecoratorInNgContainer` | Only '*' bindings are supported on <ng-container> | `RP:103` |
| `invalidDecoratorInNgContent` | Only 'select' is a valid attribute/decorate in <ng-content> | `RP:536` |
| `invalidDecoratorInTemplate` | Invalid decorator in 'template' element | `RP:371,395` |
| `invalidLetBindingInNoTemplate` | 'let-' binding can only be used in 'template' element | `RP:354` |
| `invalidMicroExpression` | Failed parsing micro expression | `MP:143-150`, `MS:332-339` |
| `nonVoidElementUsingVoidEnd` | Element is not a void-element | `RP:415,860` |
| `ngContentMustCLoseImmediately` | '<ng-content ...>' must be followed immediately by close '</ng-content>' | `RP:558,570` |
| `propertyNameTooManyFixes` | Property name can only be in format: 'name[.postfix[.unit]] | `RP:194` |
| `referenceIdentifierFound` | Reference decorator only supports #<variable> on <ng-content> | `RP:527` |
| `suffixBanana` | Expected closing banana ')]' | `SC:1060` |
| `suffixEvent` | Expected closing parenthesis ')' | `SC:1090` |
| `suffixProperty` | Expected closing bracket ']' | `SC:1122` |
| `enclosedQuote` | Expected close quote for element decorator value | `SC:623,629` |
| `unopenedMustache` | Unopened mustache | `SC:485`, `RP:658` |
| `unterminatedComment` | Unterminated comment | `SC:178,504` |
| `unterminatedMustache` | Unterminated mustache | `SC:422,855`, `RP:620,681` |
| `voidElementInCloseTag` | Void element identifiers cannot be used in close element tag | `RP:49` |
| `voidCloseInCloseTag` | Void close '/>' cannot be used in a close element | `SC:339,782` |

O scanner não repete erro para o mesmo token (`_lastErrorToken`, `SC:1195-1199`).

### R12.2 — `BuildError`s do template parser do ngcompiler
- `ADN:227-230` `select` sem valor; `ATP:309-327` três mensagens de
  `@skipOnPushValidation`; `ATP:594-605` propriedade sem entrada em `<template>`;
  `ATP:1140-1142` *"Attempted to internationalize "X", but no matching attribute or property
  found"*; `ATP:1200-1206` *warning* de elemento filtrado; `ATP:1400-1512` validações de
  R5.3; `TP:59-116` as de R7.4; `ATP:1543-1557` pipe inexistente / argumentos demais;
  `MDV:48-54,65-73,165-174,222-232` as de R11; exceções de parse de expressão
  (`ParseException`) viram `BuildError` com o `toString()` (`ATP:370-375`, `507-512`,
  `543-548`, `616-621`, `653-658`).

**Porte:** não reproduz nenhuma mensagem; nos casos que detecta, **recusa** o componente
com uma `Recusa` própria. Aceita silenciosamente a maioria dos erros de parse. — **FALTA**
(para um porte byte a byte que também reporte erros).

---

## Lacunas do porte (priorizadas)

1. **Laço infinito em `<` que não abre tag** (`H:nos`/`H:ler_texto`, `html.rs:247-279`,
   `319-321`): `a < b`, `<3`, `{{ a < b }}` travam o gerador. O oficial trata `<` sempre como
   início de tag (erro `elementIdentifier`) — exceto dentro de `{{ }}`, onde é texto da
   expressão (R1.3, R1.4). Corrigir primeiro: afeta qualquer template com comparação dentro
   de interpolação.
2. **`&nbsp;` aparado na minimização** (`H:colapsar`, `html.rs:705-710`): `trim_start`/
   `trim_end` do Rust removem `U+00A0`; o oficial o protege (`U+E501`, `WS:117-126`).
   `<div>&nbsp;</div>` perde o nó de texto; gera visão diferente (R6.4–R6.6).
3. **Entidades não decodificadas no valor da interpolação** (`html.rs:334-338`): o oficial
   decodifica (`ST:318-337`), p. ex. `{{ a &amp;&amp; b }}` (R1.5).
4. **Validação do esquema DOM ausente** (`_schema`, `hasProperty`, `hasAttribute`,
   `hasEvent`, R7.4, R10, R11): `[foo]` em `<div>`, `[for]` em `<label>`, `<div data-x>`,
   `(nãoNativo)` compilam no porte e são erro no oficial. Portar a tabela verbatim de R10.5 e
   o `MissingDirectiveValidator` (com as allowlists de `SSV`, `aria_attributes.dart`,
   `html_events.dart`).
5. **Namespaces (`svg`, `math`, `ns:x`)** (R5.1): sem `_NamespaceVisitor`, `@svg:*` nunca
   aparece; nome de elemento, descritor de seletor e chaves do esquema/segurança divergem.
6. **Distinção entre atributo sem valor e `x=""`** (R3.3, R4.1, R4.2, R8.1): perdida em
   `H:ler_atributo` (`html.rs:433-437`). Afeta `*foo` × `*foo=""` (atributo × propriedade
   vazia), `[(x)]` sem valor (descartado no oficial), `[x=""]` em seletor, e o erro de
   `<ng-content select>`.
7. **`<ng-content>` incompleto** (R3.5, R8.6): `ngProjectAs` e `#ref` descartados em
   silêncio; `ngProjectAs` muda o `ngContentIndex` do conteúdo projetado no pai.
8. **Filtro de `<script>`, `<style>` e `<link href>`** (R5.2): o oficial remove esses nós
   (com *warning*); o porte os mantém.
9. **Microssintaxe tolerante demais** (R4.4–R4.5): `M:partes` respeita aspas/parênteses e
   ignora partes desconhecidas; o oficial separa por `;` cru e acusa
   `invalidMicroExpression` (`else t`, `index as i`, `chave : x`, `f(';')`).
10. **Caixa de nomes**: o porte fecha tags e reconhece `ng-content`/vazios sem distinguir
    maiúsculas e passa a tag em minúsculas ao `V:saneador`; o oficial compara exato (tags,
    `ng-content`, `ng-container`, vazios, `pre`, chave de segurança) e só usa minúsculas em
    `template`, `script`/`style`, em-linha e `hasProperty` (R3.1, R3.2, R3.6, R10.2).
11. **Lista de vazios** sem `command`/`keygen`, e `/>` aceito em elemento não vazio
    (`html.rs:13-16`, `380-384`; R3.1).
12. **`#ref="x"`** (R9.1): oficial pega a **primeira** diretiva com `exportAs == x` e, sem
    nenhuma, liga ao elemento; o porte recusa ambos os casos (seguro, mas limita).
13. **DOCTYPE** vira `TextAst` no oficial (R1.2); o porte o descarta.
14. **Validações de R5.3 e erros de R12** (atributos/propriedades duplicados, `-` em
    referência, `:` em evento, evento sem valor, anotação inválida, `let-` fora de
    `<template>`, `*` duplicado, etc.): nenhuma, exceto eventos duplicados.
15. **Escape `\"`/`\'` em valor entre aspas** (R1.6): o oficial troca e aceita aspa
    escapada dentro do valor; o porte termina o valor na primeira aspa.
16. **Classes de espaço** `\s`/`trim` do Dart versus Rust (`U+0085`, `U+FEFF`) (R6.6) —
    borda.

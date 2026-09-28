# dartforge-sass

Compilador Sass do DartForge. É o [`grass_compiler`](https://github.com/connorskees/grass)
**0.13.4** (MIT, © 2020 Connor Skees — ver `LICENSE`), trazido para o repositório
(`src/` era o `src/` do pacote publicado no crates.io) e mantido aqui para sair
**byte a byte igual ao dart-sass 1.102.0**, a versão que o `sass_builder` 2.2.1 usa nos
projetos. O grass segue a estrutura do dart-sass (parser, avaliador, serializador), então
cada correção aponta a regra do dart-sass que ela reproduz, em comentário no código.

## O que mudou em relação ao grass 0.13.4

* **Importador do `sass_builder`** (`src/sass_builder.rs`, `src/importer.rs`): entrada
  como ativo do `build_runner` (`package:`/`asset:`), resolução pelo `BuildImporter`
  (parcial `_x`, `.sass`/`.scss`, `index`, ambiguidade é erro, `package:` pelo
  `package_config.json`), cache de folhas por URL canônica como o `ImportCache`.
* **Serializador** (`src/serializer.rs`): porte do `_SerializeVisitor` — disposição das
  regras, comentários à direita, `isGroupEnd`, indentação de seletores em várias linhas,
  números (`_writeNumber`/`_writeRounded`), strings (escapes, caracteres de uso
  privado), valores crus de propriedades customizadas, `@media`/`@supports`/`@import`.
* **Mapa de fontes** (`src/mapa.rs`): porte do `SourceMapBuffer` e do `SingleMapping`
  (`source_maps` 0.10), com os `span`s dos nós CSS e os nós de variáveis
  (`valueSpanForMap`) do avaliador; `sources` reescritas como o `sass_builder` faz.
* **Cores** (`src/color/`, `src/builtin/functions/color/`): porte do modelo de cor do
  dart-sass 1.79+ (espaços do CSS Color 4, canais ausentes, sem arredondar) e de
  `functions/color.dart`.
* **Avaliador**: `_copyParentAfterSibling`, `hasFollowingSibling` só com irmãos visíveis,
  valores de função CSS sempre serializados no estilo `expanded`, e outras — cada uma
  comentada no lugar.
* Sem `unsafe` (o workspace o nega): o internador de nomes vaza os textos em vez de usar
  ponteiro cru.

## Conferência

`examples/conferir.rs` compila uma lista de `.scss` e compara, byte a byte, o `.css` e o
`.css.map` com as saídas de um oráculo — o `SassBuilder` reproduzido num programa Dart com
o pacote `sass` 1.102.0 (mesmas opções: `compileStringToResult` com o `BuildImporter`,
`url: inputId.uri`, `sourceMap`, os dois estilos).

## Modo de compatibilidade com o dart-sass 1.66.0

`Options::versao(VersaoDartSass::V1_66_0)` imita o dart-sass **1.66.0** — o `sass` do lock
de pacotes que não resolvem com o 1.102 (o ngcomponents 3.0.0-dev.1). O motor escolhe o modo
pela versão do `sass` no `pubspec.lock` (`descritor::imita` aceita 1.102.0 e 1.66.0). As
diferenças portadas, lidas do código das duas versões e do `CHANGELOG`:

* **"mixed-decls"** (1.92.0): o 1.66 não tem o `_copyParentAfterSibling` — declaração,
  comentário, `@import` e at-rule sem corpo depois de uma regra aninhada (inclusive vindos
  de `@include`) vão para o bloco do seletor pai, antes da regra aninhada.
* **Cores** (1.79.0 e 1.101.4): o modelo do 1.66 (`src/color/v166.rs`) — RGB inteiro de
  0 a 255 ou HSL com o RGB arredondado (`fuzzyRound`), formatos `rgbFunction`,
  `hslFunction` e o texto original —, as funções de cor do 1.66
  (`src/builtin/functions/color/v166.rs`: `mix` arredonda, `darken` e afins guardam o HSL,
  `rgb()` corta os canais, `adjust/scale/change-color` do `_updateComponents` antigo…) e o
  `visitColor` do 1.66 no serializador (`rgba(r, g, b, a)` com inteiros, `hsl()` só para o
  que saiu de `hsl()`).
* **Cálculos** (1.67.0): só `calc()`, `clamp()`, `min()` e `max()` são cálculos; `round()`
  e `abs()` são as funções do Sass e `sin()`, `mod()`… funções CSS puras. Interpolação no
  nível de cima de um argumento ou de um `(…)` dentro dele vira o texto cru
  (`CalculationInterpolation`, `AstExpr::CalcInterp166`), entre parênteses como operando;
  `(x)` perde os parênteses (exceto `(var(…))`); lista separada por espaço é erro.
* **`@extend`/`selector.unify`** (1.79.5): `unifyCompound` antigo (os simples do primeiro
  entram no segundo). O modo padrão passou a usar o do 1.102 (pseudo-classes depois de um
  pseudo-elemento ficam depois dele).
* Números: sem precisão total no `inspect` (1.91.0) e unidade complexa é erro fora do
  `inspect` (1.96.0); string: U+007F sem escape (1.69.6); propriedade customizada vazia é
  erro (1.88.0); `//` fica no valor cru de funções especiais (1.77.7 — o modo padrão agora
  o trata como comentário silencioso).

Recusado com erro que o diz (a saída do 1.66 não é garantida): cor fora do modelo do 1.66
(espaços do CSS Color 4, canais ausentes, RGB fracionário) e as funções que o 1.66 não tinha
(`lab()`, `oklch()`, `color()`, `hwb()` global, `color.channel()`, `color.to-space()`…).
Ficam de fora (não observados; ver o `CHANGELOG`): a gramática de cálculo inteira do 1.66
além da interpolação, `meta.apply`/`get-mixin` (1.69) e sintaxe que o 1.66 recusava e o port
aceita (vírgula final, `%` solto, aninhamento em CSS puro).

Conferência: `tests/formas_166.rs` compara cada forma de `tests/formas_166/fontes` com o
dart-sass 1.66.0 e 1.102.0 de verdade (`scripts/sass-formas-166.sh` grava os oráculos em
`esperado/` e `esperado_102/`). No ngcomponents 3.0.0-dev.1, as 70 `.scss.css` saem iguais
às do `build_runner` com o sass 1.66.0.

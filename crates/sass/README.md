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

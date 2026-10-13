# dartforge-sass

The DartForge Sass compiler. It is [`grass_compiler`](https://github.com/connorskees/grass)
**0.13.4** (MIT, © 2020 Connor Skees — see `LICENSE`), brought into the repository
(`src/` was the `src/` of the package published on crates.io) and maintained here to come
out **byte for byte equal to dart-sass 1.102.0**, the version that `sass_builder` 2.2.1 uses
in the projects. grass follows the structure of dart-sass (parser, evaluator, serializer),
so each fix points, in a comment in the code, to the dart-sass rule it reproduces.

## What changed relative to grass 0.13.4

* **`sass_builder` importer** (`src/sass_builder.rs`, `src/importer.rs`): the input as a
  `build_runner` asset (`package:`/`asset:`), resolution by the `BuildImporter` (partial
  `_x`, `.sass`/`.scss`, `index`, ambiguity is an error, `package:` through
  `package_config.json`), a stylesheet cache by canonical URL like `ImportCache`.
* **Serializer** (`src/serializer.rs`): port of `_SerializeVisitor` — layout of the rules,
  trailing comments, `isGroupEnd`, indentation of multi-line selectors, numbers
  (`_writeNumber`/`_writeRounded`), strings (escapes, private-use characters), raw values of
  custom properties, `@media`/`@supports`/`@import`.
* **Source map** (`src/source_map.rs`): port of `SourceMapBuffer` and `SingleMapping`
  (`source_maps` 0.10), with the `span`s of the CSS nodes and the variable nodes
  (`valueSpanForMap`) from the evaluator; `sources` rewritten as `sass_builder` does.
* **Colors** (`src/color/`, `src/builtin/functions/color/`): port of the dart-sass 1.79+
  color model (CSS Color 4 spaces, missing channels, no rounding) and of
  `functions/color.dart`.
* **Evaluator**: `_copyParentAfterSibling`, `hasFollowingSibling` with visible siblings
  only, CSS function values always serialized in the `expanded` style, and others — each
  one commented in place.
* No `unsafe` (the workspace denies it): the name interner leaks the strings instead of
  using a raw pointer.

## Checking

`examples/check.rs` compiles a list of `.scss` files and compares, byte for byte, the
`.css` and the `.css.map` with the outputs of an oracle — `SassBuilder` reproduced in a Dart
program with the `sass` 1.102.0 package (same options: `compileStringToResult` with the
`BuildImporter`, `url: inputId.uri`, `sourceMap`, both styles).

## Compatibility mode with dart-sass 1.66.0

`Options::version(VersaoDartSass::V1_66_0)` imitates dart-sass **1.66.0** — the `sass` in
the lock of packages that do not resolve with 1.102 (ngcomponents 3.0.0-dev.1). The engine
chooses the mode by the version of `sass` in `pubspec.lock` (`descritor::imita` accepts
1.102.0 and 1.66.0). The ported differences, read from the code of both versions and from
the `CHANGELOG`:

* **"mixed-decls"** (1.92.0): 1.66 does not have `_copyParentAfterSibling` — a
  declaration, comment, `@import` or body-less at-rule after a nested rule (including those
  coming from `@include`) goes into the parent selector's block, before the nested rule.
* **Colors** (1.79.0 and 1.101.4): the 1.66 model (`src/color/v166.rs`) — integer RGB from
  0 to 255, or HSL with the rounded RGB (`fuzzyRound`), the `rgbFunction` and
  `hslFunction` formats and the original text —, the 1.66 color functions
  (`src/builtin/functions/color/v166.rs`: `mix` rounds, `darken` and the like keep the HSL,
  `rgb()` clamps the channels, `adjust/scale/change-color` from the old
  `_updateComponents`…) and the 1.66 `visitColor` in the serializer (`rgba(r, g, b, a)`
  with integers, `hsl()` only for what came out of `hsl()`).
* **Calculations** (1.67.0): only `calc()`, `clamp()`, `min()` and `max()` are
  calculations; `round()` and `abs()` are the Sass functions and `sin()`, `mod()`… plain
  CSS functions. Interpolation at the top level of an argument or of a `(…)` inside it
  becomes the raw text (`CalculationInterpolation`, `AstExpr::CalcInterp166`), in
  parentheses as an operand; `(x)` loses the parentheses (except `(var(…))`); a
  space-separated list is an error.
* **`@extend`/`selector.unify`** (1.79.5): the old `unifyCompound` (the simple selectors of
  the first go into the second). The default mode now uses the 1.102 one (pseudo-classes
  after a pseudo-element stay after it).
* Numbers: no full precision in `inspect` (1.91.0) and a complex unit is an error outside
  `inspect` (1.96.0); strings: U+007F unescaped (1.69.6); an empty custom property is an
  error (1.88.0); `//` stays in the raw value of special functions (1.77.7 — the default
  mode now treats it as a silent comment).

Refused with an error that says so (the 1.66 output is not guaranteed): a color outside the
1.66 model (CSS Color 4 spaces, missing channels, fractional RGB) and the functions that
1.66 did not have (`lab()`, `oklch()`, `color()`, global `hwb()`, `color.channel()`,
`color.to-space()`…). Left out (not observed; see the `CHANGELOG`): the whole 1.66
calculation grammar beyond interpolation, `meta.apply`/`get-mixin` (1.69) and syntax that
1.66 refused and the port accepts (trailing comma, lone `%`, nesting in plain CSS).

Checking: `tests/formas_166.rs` compares each shape in `tests/formas_166/fontes` with the
real dart-sass 1.66.0 and 1.102.0 (`scripts/sass-formas-166.sh` records the oracles in
`esperado/` and `esperado_102/`). In ngcomponents 3.0.0-dev.1, the 70 `.scss.css` files come
out equal to those of `build_runner` with sass 1.66.0.

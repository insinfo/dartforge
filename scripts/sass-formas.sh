#!/usr/bin/env bash
# Regenera o oráculo das formas Sass (crates/gerador_ng/tests/sass_formas):
# o `sass_builder` 2.2.1 de verdade, pelo `build_runner`, com as versões do
# lock de corpus/builders/sass_builder (dart-sass 1.102.0), nos dois estilos
# (`outputStyle: expanded` e `compressed`), sem mapas de fonte.
#
# Para cada forma `fontes/fNN_*.scss` grava `esperado/fNN_*.<estilo>.css`
# (os bytes que o builder escreveu) ou `esperado/fNN_*.<estilo>.erro` (o
# builder falhou: a forma é inválida para o oficial e o nativo tem de
# recusar). Os `.scss` que não começam com `f` são módulos das formas.
#
# Uso: scripts/sass-formas.sh [dart]   (padrão: `dart` do PATH, SDK 3.6.2)
set -euo pipefail
dart=${1:-dart}
raiz=$(cd "$(dirname "$0")/.." && pwd)
formas=$raiz/crates/gerador_ng/tests/sass_formas
caso=$raiz/corpus/builders/sass_builder
mkdir -p "$formas/esperado"
for estilo in expanded compressed; do
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  cp "$caso/pubspec.yaml" "$caso/pubspec.lock" "$tmp/"
  mkdir -p "$tmp/lib/formas"
  # Os módulos em sintaxe indentada (`.sass`) também: sem eles a forma que
  # os usa falha por arquivo ausente, não pelo Sass.
  cp "$formas"/fontes/*.scss "$formas"/fontes/*.sass "$tmp/lib/formas/"
  printf 'targets:\n  $default:\n    builders:\n      sass_builder:\n        options:\n          outputStyle: %s\n          sourceMaps: false\n' "$estilo" > "$tmp/build.yaml"
  (cd "$tmp" && "$dart" pub get --offline --enforce-lockfile > pub.log 2>&1)
  # Uma forma inválida faz o build terminar com erro; as outras saem mesmo
  # assim.
  (cd "$tmp" && "$dart" run build_runner build --delete-conflicting-outputs > build.log 2>&1) || true
  gerado=$tmp/.dart_tool/build/generated/corpus_sass_builder/lib/formas
  for f in "$formas"/fontes/f*.scss; do
    n=$(basename "$f" .scss)
    rm -f "$formas/esperado/$n.$estilo.css" "$formas/esperado/$n.$estilo.erro"
    if [ -f "$gerado/$n.css" ]; then
      cp "$gerado/$n.css" "$formas/esperado/$n.$estilo.css"
    else
      : > "$formas/esperado/$n.$estilo.erro"
    fi
  done
  rm -rf "$tmp"
  trap - EXIT
done
echo "oráculo gravado em $formas/esperado"

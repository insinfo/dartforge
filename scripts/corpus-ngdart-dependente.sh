#!/usr/bin/env bash
# Regenera o oráculo de corpus/ngdart_dependente: o `build_runner` oficial
# (ngdart 8.0.0-dev.4 com `build_web_compilers`, que pede as saídas opcionais
# do ngdart) numa cópia do caso, e os `.template.dart` dos dois pacotes
# (`app_ng` e a dependência `path` `dep_ng`) copiados de
# `.dart_tool/build/generated/<pacote>/`.
#
# Uso: scripts/corpus-ngdart-dependente.sh [dart]   (padrão: `dart` do PATH)
set -euo pipefail
dart=${1:-dart}
raiz=$(cd "$(dirname "$0")/.." && pwd)
caso=$raiz/corpus/ngdart_dependente
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
cp -r "$caso/app" "$caso/dep" "$tmp/"
(cd "$tmp/app" && "$dart" pub get --enforce-lockfile > "$tmp/pub.log" 2>&1)
(cd "$tmp/app" && "$dart" run build_runner build --delete-conflicting-outputs > "$tmp/build.log" 2>&1)
rm -rf "$caso/oraculo"
for pacote in app_ng dep_ng; do
  (cd "$tmp/app/.dart_tool/build/generated" && find "$pacote" -name '*.template.dart') | while read -r rel; do
    mkdir -p "$caso/oraculo/$(dirname "$rel")"
    cp "$tmp/app/.dart_tool/build/generated/$rel" "$caso/oraculo/$rel"
  done
done
echo "oráculo gravado em $caso/oraculo"

#!/usr/bin/env bash
# Regenera o oráculo das formas do modo de compatibilidade com o dart-sass
# 1.66.0 (crates/sass/tests/formas_166): o pacote `sass` de verdade, pelo
# `compileToResult`, nos dois estilos, sem mapas de fonte — o 1.66.0 (o do
# lock do ngcomponents 3.0.0-dev.1) em `esperado/` e o 1.102.0 (o do
# `sass_builder` 2.2.1) em `esperado_102/`, para as mesmas formas.
#
# Para cada `fontes/<forma>.scss` grava `<dir>/<forma>.<estilo>.css` (o
# `CompileResult.css`, sem `\n` final) ou `<dir>/<forma>.erro` (a mensagem
# do dart-sass: a forma é inválida para ele). As formas `f*` o modo 1.66 tem
# de reproduzir byte a byte; as `r*` compilam no 1.66 mas o modo as recusa
# (construções cuja saída ele não garante).
#
# Uso: scripts/sass-formas-166.sh [dart]   (padrão: `dart` do PATH; precisa
# do `sass` 1.66.0 e 1.102.0 no cache do pub — `dart pub get --offline`).
set -euo pipefail
dart=${1:-dart}
raiz=$(cd "$(dirname "$0")/.." && pwd)
formas=$raiz/crates/sass/tests/formas_166
for par in "1.66.0 esperado" "1.102.0 esperado_102"; do
  set -- $par
  versao=$1
  destino=$formas/$2
  tmp=$(mktemp -d)
  mkdir -p "$tmp/bin" "$destino"
  cat > "$tmp/pubspec.yaml" <<YAML
name: oraculo_sass
environment:
  sdk: ^3.0.0
dependencies:
  sass: $versao
YAML
  cat > "$tmp/bin/oraculo.dart" <<'DART'
import 'dart:io';
import 'package:sass/sass.dart' as sass;

/// `oraculo.dart <destino> <forma.scss>...`
void main(List<String> args) {
  final saida = args.first;
  for (final f in args.skip(1)) {
    final nome = f.split('/').last.replaceAll(RegExp(r'\.scss$'), '');
    for (final e in [File('$saida/$nome.expanded.css'),
        File('$saida/$nome.compressed.css'), File('$saida/$nome.erro')]) {
      if (e.existsSync()) e.deleteSync();
    }
    try {
      for (final (estilo, sufixo) in [
        (sass.OutputStyle.expanded, 'expanded'),
        (sass.OutputStyle.compressed, 'compressed'),
      ]) {
        final r = sass.compileToResult(f, style: estilo, logger: sass.Logger.quiet);
        File('$saida/$nome.$sufixo.css').writeAsStringSync(r.css);
      }
    } on sass.SassException catch (e) {
      for (final s in ['expanded', 'compressed']) {
        final c = File('$saida/$nome.$s.css');
        if (c.existsSync()) c.deleteSync();
      }
      File('$saida/$nome.erro').writeAsStringSync('${e.message}\n');
    }
  }
}
DART
  (cd "$tmp" && "$dart" pub get --offline > pub.log 2>&1)
  (cd "$tmp" && "$dart" run bin/oraculo.dart "$destino" "$formas"/fontes/*.scss)
  rm -rf "$tmp"
  echo "oráculo do dart-sass $versao gravado em $destino"
done

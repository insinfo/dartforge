#!/usr/bin/env bash
# Regenera o oráculo do corpus do ngdart sem PowerShell: o mesmo roteiro de
# scripts/corpus-ngdart.ps1 — gera o `web/main.dart` e o
# `lib/corpus_ngdart.dart` a partir dos casos presentes, roda o
# `build_runner` oficial (offline, com o pub-cache local) e copia os
# `.template.dart`, `.css.shim.dart` e `.css.dart` para `oraculo/`.
#
# Uso: scripts/corpus-ngdart.sh [--limpar]
set -euo pipefail

raiz="$(cd "$(dirname "$0")/../corpus/ngdart" && pwd)"
cd "$raiz"

# A ordem é a do `Sort-Object Name` do script PowerShell; os nomes do corpus
# são minúsculos, com dígitos e `_`, e nenhum par deles depende da ordem
# entre `_` e dígito — `LC_ALL=C` dá a mesma lista.
todos=$(cd lib/src && ls -1 | grep '\.dart$' | grep -v '\.template\.dart$' | LC_ALL=C sort)

# 1. main.dart com um import por componente, para o builder opcional rodar.
{
    echo '// Consome os `.template.dart` do corpus. O builder do ngdart é'
    echo '// `is_optional: true`: sem alguém pedindo a saída, ele não roda.'
    echo '// Gerado por scripts/corpus-ngdart.ps1.'
    fabricas=()
    for arquivo in $todos; do
        grep -q '@Component(' "lib/src/$arquivo" || continue
        nome="${arquivo%.dart}"
        prefixo="${nome%%_*}"
        classe=""
        IFS='_' read -ra partes <<< "$nome"
        for p in "${partes[@]}"; do
            classe+="${p^}"
        done
        echo "import 'package:corpus_ngdart/src/$nome.template.dart' as $prefixo;"
        fabricas+=("    $prefixo.${classe}NgFactory,")
    done
    echo ''
    echo 'void main() {'
    echo '  print(['
    printf '%s\n' "${fabricas[@]}"
    echo '  ].length);'
    echo '}'
} > web/main.dart

# 1b. a biblioteca que reexporta todos os casos.
{
    echo '/// Biblioteca do corpus: existe para a carga alcançar todos os casos numa'
    echo '/// entrada só — é por ela que o teste monta o banco semântico.'
    echo 'library corpus_ngdart;'
    echo ''
    for arquivo in $todos; do
        echo "export 'src/$arquivo';"
    done
} > lib/corpus_ngdart.dart

# 2. o compilador oficial.
dart pub get --offline
dart run build_runner build --delete-conflicting-outputs

# 3. o oráculo.
gerado=".dart_tool/build/generated/corpus_ngdart/lib/src"
mkdir -p oraculo
cp "$gerado"/*.template.dart oraculo/
for extensao in css.shim.dart css.dart; do
    if compgen -G "$gerado/*.$extensao" > /dev/null; then
        cp "$gerado"/*."$extensao" oraculo/
    fi
done
echo "oráculo atualizado: $(ls oraculo | wc -l) arquivos"
if [[ "${1:-}" == "--limpar" ]]; then
    rm -rf .dart_tool/build
fi

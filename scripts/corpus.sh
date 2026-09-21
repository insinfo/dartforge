#!/bin/sh
# Baixa pacotes Dart reais do pub.dev para aferir o compilador contra produção.
#
# O corpus não é versionado: é grande e tem licença própria de cada pacote.
# `references/` já é ignorado pelo Git. Rode este script uma vez; o teste
# `corpus_real` encontra o que estiver baixado e é ignorado se não houver nada.
set -e
cd "$(dirname "$0")/.."
DESTINO="references/pub"
mkdir -p "$DESTINO"
for PACOTE in "$@"; do
  VERSAO=$(curl -s "https://pub.dev/api/packages/$PACOTE" \
    | python -c "import sys,json;print(json.load(sys.stdin)['latest']['version'])")
  ALVO="$DESTINO/$PACOTE-$VERSAO"
  if [ -d "$ALVO" ]; then
    echo "$PACOTE $VERSAO já baixado"
    continue
  fi
  echo "baixando $PACOTE $VERSAO"
  curl -sL -o "$DESTINO/$PACOTE-$VERSAO.tar.gz" \
    "https://pub.dev/api/archives/$PACOTE-$VERSAO.tar.gz"
  mkdir -p "$ALVO"
  tar -xzf "$DESTINO/$PACOTE-$VERSAO.tar.gz" -C "$ALVO"
  echo "$PACOTE $VERSAO: $(find "$ALVO" -name '*.dart' | wc -l) arquivos Dart"
done

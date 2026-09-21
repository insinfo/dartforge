#!/bin/sh
# Baixa pacotes Dart reais do pub.dev para aferir o compilador contra produção.
#
# O corpus não é versionado: é grande e tem licença própria de cada pacote.
# `references/` já é ignorado pelo Git. Rode este script uma vez; o teste
# `corpus_real` encontra o que estiver baixado e é ignorado se não houver nada.
#
# Sem argumentos, baixa o corpus padrão: pacotes de domínios diferentes, porque
# um corpus de um único pacote mede o estilo daquele pacote, não a linguagem.
#
# Com `--com-dependencias`, segue as dependências declaradas em cada pubspec.
# É o que permite aferir pelo grafo: sem os pacotes importados no disco, todo
# `package:` falha na carga e a medição vira um relatório de downloads ausentes.
#
# Ao final sempre escreve `references/pub/.dart_tool/package_config.json`
# (formato v2, lido por `crates/packages`) mapeando cada pacote baixado para o
# seu nome, de modo que `package:pdf/pdf.dart` resolva a partir de qualquer
# arquivo do corpus.
set -e
cd "$(dirname "$0")/.."
DESTINO="references/pub"
mkdir -p "$DESTINO"

# Rede, serialização e internacionalização ao lado da geração de PDF.
CORPUS_PADRAO="pdf http collection intl"
COM_DEPENDENCIAS=0
FILA=""
for ARGUMENTO in "$@"; do
  case "$ARGUMENTO" in
    --com-dependencias) COM_DEPENDENCIAS=1 ;;
    -*)
      echo "opção desconhecida: $ARGUMENTO" >&2
      exit 2
      ;;
    *) FILA="$FILA $ARGUMENTO" ;;
  esac
done
[ -n "$(echo "$FILA" | tr -d ' ')" ] || FILA="$CORPUS_PADRAO"

# Dependências de um pubspec sem um parser de YAML: as chaves indentadas sob
# `dependencies:`, que é a forma que todo pacote do pub.dev usa. `flutter` é
# descartada porque não vem do pub e traria um SDK inteiro.
dependencias() {
  awk '
    /^dependencies:/ { dentro = 1; next }
    /^[a-zA-Z_]+:/ { dentro = 0 }
    dentro && /^  [a-z_0-9]+:/ { sub(/:.*/, ""); gsub(/ /, ""); print }
  ' "$1" | grep -v '^flutter$' || true
}

BAIXADOS=""
while [ -n "$(echo "$FILA" | tr -d ' ')" ]; do
  PACOTE=$(echo "$FILA" | tr ' ' '\n' | grep -v '^$' | head -1)
  FILA=$(echo "$FILA" | tr ' ' '\n' | grep -v '^$' | tail -n +2 | tr '\n' ' ')
  case " $BAIXADOS " in
    *" $PACOTE "*) continue ;;
  esac
  BAIXADOS="$BAIXADOS $PACOTE"
  VERSAO=$(curl -s "https://pub.dev/api/packages/$PACOTE" \
    | python -c "import sys,json
try:
    print(json.load(sys.stdin)['latest']['version'])
except Exception:
    pass")
  if [ -z "$VERSAO" ]; then
    # Nome que o pub.dev não conhece: quase sempre uma dependência de SDK
    # (`flutter_test`) ou um apelido. Avisar e seguir é melhor do que abortar
    # o corpus inteiro por causa de uma folha da árvore de dependências.
    echo "aviso: $PACOTE não encontrado no pub.dev; ignorado" >&2
    continue
  fi
  ALVO="$DESTINO/$PACOTE-$VERSAO"
  if [ -d "$ALVO" ]; then
    echo "$PACOTE $VERSAO já baixado"
  else
    echo "baixando $PACOTE $VERSAO"
    curl -sL -o "$DESTINO/$PACOTE-$VERSAO.tar.gz" \
      "https://pub.dev/api/archives/$PACOTE-$VERSAO.tar.gz"
    mkdir -p "$ALVO"
    tar -xzf "$DESTINO/$PACOTE-$VERSAO.tar.gz" -C "$ALVO"
    echo "$PACOTE $VERSAO: $(find "$ALVO" -name '*.dart' | wc -l) arquivos Dart"
  fi
  if [ "$COM_DEPENDENCIAS" = 1 ] && [ -f "$ALVO/pubspec.yaml" ]; then
    FILA="$FILA $(dependencias "$ALVO/pubspec.yaml" | tr '\n' ' ')"
  fi
done

# O mapa nome → diretório é reescrito a cada rodada: um pacote baixado depois
# precisa entrar, e um `package_config` desatualizado esconderia a resolução
# justamente do pacote novo.
CONFIG="$DESTINO/.dart_tool/package_config.json"
mkdir -p "$DESTINO/.dart_tool"
{
  printf '{"configVersion":2,"packages":['
  SEPARADOR=""
  for CAMINHO in "$DESTINO"/*/; do
    DIRETORIO=$(basename "$CAMINHO")
    [ "$DIRETORIO" = ".dart_tool" ] && continue
    [ -d "$CAMINHO/lib" ] || continue
    # Nomes de pacote do pub não têm hífen, então o primeiro `-` seguido de
    # dígito começa a versão, inclusive em pré-lançamentos como `1.0.0-beta.1`.
    NOME=${DIRETORIO%%-[0-9]*}
    printf '%s{"name":"%s","rootUri":"../%s/","packageUri":"lib/","languageVersion":"3.6"}' \
      "$SEPARADOR" "$NOME" "$DIRETORIO"
    SEPARADOR=","
  done
  printf ']}\n'
} > "$CONFIG"
echo "package_config: $CONFIG"

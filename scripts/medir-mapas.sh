#!/usr/bin/env bash
# Etapa 2 dos mapas de pilha (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §6 e §8):
# para cada programa de bench/desempenho, em A0 (pilha-sombra) e em B0
# (mapas),
#
# * o tempo da compilação de desenvolvimento (`compile-native`, sem cache de
#   objetos; o SDK compilado fica no cache dele, como no uso normal);
# * o tamanho do executável de produção (`aot --optimize`, o programa e o SDK
#   ligados juntos) por seção: `.text`, `.rdata`, `.pdata` e o mapa
#   (`.dfgcm`).
#
# A0 em produção vai pela LTO completa do ligador e B0 pelo ThinLTO
# distribuído; com `DISTRIBUIDA=1`, A0 também vai pelo distribuído
# (`DARTFORGE_LTO_DISTRIBUIDA=1`), e a diferença fica só no modo de raízes.
#
# Uso: [DISTRIBUIDA=1] scripts/medir-mapas.sh <dir de saída> [llvm-objdump]
set -eu
saida=${1:?diretório de saída}
objdump=${2:-llvm-objdump}
raiz=$(cd "$(dirname "$0")/.." && pwd)
mkdir -p "$saida"
printf 'programa\tmodo\tdev_s\tarquivo\t.text\t.rdata\t.pdata\t.dfgcm\n'
for f in "$raiz"/bench/desempenho/*.dart; do
  nome=$(basename "$f" .dart)
  [ "$nome" = comum ] && continue
  for modo in sombra mapas; do
    dev="$saida/$nome-$modo-dev.exe"
    exe="$saida/$nome-$modo.exe"
    ini=$(date +%s.%N)
    DARTFORGE_CACHE_OBJ=0 DARTFORGE_RAIZES=$modo "$raiz/target/release/dartforge" compile-native "$f" -o "$dev" >/dev/null
    fim=$(date +%s.%N)
    DARTFORGE_LTO_DISTRIBUIDA=${DISTRIBUIDA:-0} DARTFORGE_RAIZES=$modo "$raiz/target/release/dartforge" aot "$f" "$exe" --optimize >/dev/null
    secoes=$("$objdump" -h "$exe" | awk 'NR>4 {printf "%s=%d ", $2, strtonum("0x"$3)}')
    pega() { v=$(echo "$secoes" | tr ' ' '\n' | grep "^$1=" | cut -d= -f2 | head -1); echo "${v:-0}"; }
    printf '%s\t%s\t%.1f\t%s\t%s\t%s\t%s\t%s\n' "$nome" "$modo" "$(awk -v a="$ini" -v b="$fim" 'BEGIN{print b-a}')" "$(stat -c %s "$exe")" "$(pega .text)" "$(pega .rdata)" "$(pega .pdata)" "$(pega .dfgcm)"
  done
done

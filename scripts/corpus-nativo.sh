#!/usr/bin/env bash
# Roda corpus/nativo contra o `dart run`, vários programas ao mesmo tempo.
#
#   scripts/corpus-nativo.sh [tracing|arc|preparado|tabelas] [jobs] [filtro]
#
# `preparado` liga DARTFORGE_ARC_PREPARADO=1 (programa e SDK preparados);
# `tabelas` é o mesmo com --excecoes=tabelas.
# As saídas de referência do Dart ficam em target/corpus-dart/ e só são
# refeitas quando o .dart muda. O resumo vai para target/corpus-<modo>/resumo.tsv.
# Usa target/release/dartforge (ou DARTFORGE_BIN) e o ambiente atual
# (DARTFORGE_CLANG, LLVM no PATH etc.).
set -u
modo=${1:-preparado}
jobs=${2:-6}
filtro=${3:-}
raiz=$(cd "$(dirname "$0")/.." && pwd)
bin=${DARTFORGE_BIN:-$raiz/target/release/dartforge}
ref=$raiz/target/corpus-dart
dest=$raiz/target/corpus-$modo
mkdir -p "$ref" "$dest"

um() {
  f=$1
  n=$(basename "$f" .dart)
  dir=$(dirname "$f")
  # A referência do Dart, refeita só se o programa mudou.
  if [ ! -f "$ref/$n.out" ] || [ "$f" -nt "$ref/$n.out" ]; then
    (cd "$dir" && timeout 60 dart run "$f" > "$ref/$n.out" 2>&1 < /dev/null)
  fi
  extra=()
  memoria=""
  case $modo in
    arc) extra=(--memoria=arc); memoria=arc ;;
    preparado) extra=(--memoria=arc); memoria=arc; export DARTFORGE_ARC_PREPARADO=1 ;;
    tabelas) extra=(--memoria=arc --excecoes=tabelas); memoria=arc; export DARTFORGE_ARC_PREPARADO=1 ;;
  esac
  # `${extra[@]+…}`: o bash 3.2 do macOS recusa o vetor vazio sob `set -u`.
  if ! "$bin" compile-native "$f" -o "$dest/$n" ${extra[@]+"${extra[@]}"} > "$dest/$n.build.log" 2>&1; then
    printf "%s\tbuild\t%s\n" "$n" "$(tail -1 "$dest/$n.build.log" | cut -c1-200)"
    return
  fi
  (cd "$dir" && DARTFORGE_MEMORIA=$memoria DARTFORGE_ARC_CONFERIR=1 timeout 60 "$dest/$n" > "$dest/$n.out" 2>&1 < /dev/null)
  c=$?
  if cmp -s "$dest/$n.out" "$ref/$n.out"; then r=igual; else r=diferente; fi
  printf "%s\t%s\texit=%s\n" "$n" "$r" "$c"
}
export -f um
export modo bin ref dest

# Um programa primeiro: monta o SDK do perfil (cache) sem corrida entre jobs.
programas=$(ls "$raiz"/corpus/nativo/*.dart | grep -- "$filtro")
primeiro=$(echo "$programas" | head -1)
{
  um "$primeiro"
  echo "$programas" | tail -n +2 | xargs -P "$jobs" -I{} bash -c 'um "$@"' _ {}
} | sort > "$dest/resumo.tsv"
cut -f2 "$dest/resumo.tsv" | sort | uniq -c
grep -v "	igual	" "$dest/resumo.tsv" | cut -c1-200

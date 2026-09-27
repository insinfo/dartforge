#!/bin/sh
# Mede a matriz de suporte (V02) neste sistema: cada corpus em cada perfil
# pelo `dartforge-diferencial --matriz`, um TSV por corpus em
# `target/matriz/<sistema>/`, e depois `scripts/matriz.py` escreve a
# `docs/MATRIZ.md` com todos os sistemas medidos que estiverem ali (os TSVs de
# outro sistema — o artefato `matriz-<sistema>` do Pesado — entram copiados para
# `target/matriz/<sistema>/`).
#
# Uso: scripts/matriz.sh [--sem-nativo] [--sem-js]
# Precisa do `dartforge-diferencial` e do `dartforge` compilados em release
# (com `--features nativo,jit` para os perfis nativos) e dos oráculos
# (DARTFORGE_DART_SDK, DARTFORGE_DART_SDK_3_13 para o corpus moderno).
# O código de saída de cada rodada não interrompe o script: um programa que
# falha é um estado da matriz, não um erro da medição.
set -u
cd "$(dirname "$0")/.."
NATIVO=1
JS=1
for a in "$@"; do
    case "$a" in
        --sem-nativo) NATIVO=0 ;;
        --sem-js) JS=0 ;;
        *) echo "argumento desconhecido: $a" >&2; exit 2 ;;
    esac
done
case "$(uname -s)" in
    Linux) SO=linux ;;
    Darwin) SO=macos ;;
    MINGW*|MSYS*|CYGWIN*) SO=windows ;;
    *) SO=$(uname -s) ;;
esac
SISTEMA="$SO-$(uname -m)"
DIR="target/matriz/$SISTEMA"
D=./target/release/dartforge-diferencial
[ -x "$D" ] || [ -x "$D.exe" ] || { echo "falta $D (cargo build --release -p dartforge-diferencial)" >&2; exit 2; }
rm -rf "$DIR"
mkdir -p "$DIR"

rodar() {
    corpus_nome=$1
    shift
    echo "== $corpus_nome: $*" >&2
    "$D" --silencioso --matriz "$DIR/$corpus_nome.tsv" "$@" > "$DIR/$corpus_nome.$(echo "$*" | tr ' /' '__').log" 2>&1
    echo "   código $?" >&2
}

if [ "$JS" = 1 ]; then
    rodar js --producao --jobs 4
    rodar moderno --corpus corpus/moderno --producao
fi
if [ "$NATIVO" = 1 ]; then
    for c in js nativo; do
        corpus=corpus/$c
        rodar "$c" --corpus "$corpus" --nativo
        rodar "$c" --corpus "$corpus" --nativo --gc-stress
        rodar "$c" --corpus "$corpus" --jit
    done
fi
python3 scripts/matriz.py target/matriz docs/MATRIZ.md

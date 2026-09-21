#!/bin/sh
# Compara LLVM e Cranelift no ciclo de testes, separando compilar de executar.
#
# Duas armadilhas que este script evita de propósito:
#
# 1. Diretórios de destino distintos por backend. Sem isso, a segunda medição
#    reaproveitaria artefatos da primeira e pareceria instantânea.
# 2. Falha de compilação aborta a medição. Um `|| true` transformaria "tempo
#    até o erro" em "tempo de compilar", que é o jeito mais fácil de obter um
#    número menor pelo motivo errado — e foi o que invalidou a primeira rodada.
#
# A árvore precisa estar verde e parada. Com edições acontecendo entre as duas
# medições, os backends seriam comparados sobre códigos-fonte diferentes.
set -e
cd "$(dirname "$0")/.."
RAIZ=$(pwd)

if ! cargo +nightly check --workspace --all-targets >/dev/null 2>&1; then
  echo "árvore vermelha; medição abortada" >&2
  exit 1
fi

medir() {
  inicio=$(date +%s%N)
  if ! "$@" >"$RAIZ/target-ab-saida.txt" 2>&1; then
    echo "FALHOU: $*" >&2
    tail -5 "$RAIZ/target-ab-saida.txt" >&2
    exit 1
  fi
  fim=$(date +%s%N)
  echo $(( (fim - inicio) / 1000000 ))
}

echo "== LLVM =="
export CARGO_TARGET_DIR="$RAIZ/target-ab-llvm"
echo "compilar_testes_ms=$(medir cargo +nightly test --workspace --no-run)"
echo "executar_testes_ms=$(medir cargo +nightly test --workspace)"

echo "== Cranelift + panic-abort-tests =="
export CARGO_TARGET_DIR="$RAIZ/target-ab-clif"
export CARGO_PROFILE_DEV_CODEGEN_BACKEND=cranelift
export CARGO_PROFILE_DEV_PANIC=abort
echo "compilar_testes_ms=$(medir cargo +nightly test -Zcodegen-backend -Zpanic-abort-tests --workspace --no-run)"
echo "executar_testes_ms=$(medir cargo +nightly test -Zcodegen-backend -Zpanic-abort-tests --workspace)"

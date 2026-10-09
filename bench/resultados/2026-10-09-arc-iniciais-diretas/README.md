# Referências iniciais de ARC sem vetor temporário

Fonte runtime/emissor: `ae4fd9c5`. CLI recompilada com LLVM embutido;
hash em cli.json. Não inclui a validação posterior de NativeFinalizer.
Dois programas, quatro kernels, sete repetições A0/ARC: 28 processos,
código zero, stderr vazio e resultados iguais entre modos/repetições e
iguais à rodada anterior de instrumentação condicional.

Afinidade 0x4, heap 256 MiB, AOT --optimize, sombra/checagem, ARC puro.
GC_STRESS e ARC_CICLOS removidos; GC_RASTRO, GC_OFF, ARC_CONFERIR e
ARC_BERCARIO em 0. Dart não foi reexecutado. Não é a comparação completa.

ARC/A0: árvores 12,47; lista ligada 5,29; construção de textos 2,48;
hashes 3,31. A dispersão e o deslocamento de A0 impedem atribuir essas
razões isoladamente à remoção do vetor. ARC >= A0 permanece aberto.

Reprodução com a CLI desta fonte e as variáveis acima:

    python scripts/medir-modos-desempenho.py <saída-nova> objetos_escapam textos --repeticoes 7 --afinidade 0x4 --modos A0,ARC --sem-dart

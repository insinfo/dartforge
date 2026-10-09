# ARC com instrumentação condicional

Fonte runtime/emissor: `296c001e` (commits posteriores só atualizaram ESTADO).
CLI recompilada com LLVM embutido; hash em cli.json. Dois programas, quatro
kernels, sete repetições por A0/ARC: 28 processos, código zero, stderr vazio
e resultados idênticos. Não é a comparação completa de 32 kernels.

Afinidade 0x4, heap 256 MiB, AOT --optimize, sombra/checagem, ARC puro.
GC_STRESS e ARC_CICLOS removidos; GC_RASTRO, GC_OFF, ARC_CONFERIR e
ARC_BERCARIO em 0. Dart não foi reexecutado. Sem rastro, o novo runtime
não cronometra as etapas de drenagem. O vetor de referências iniciais ainda
existe nesta rodada; sua remoção foi preparada somente após a compilação
dos quatro executáveis e não integra esta medição.

ARC/A0: árvores 12,02, lista ligada 6,23, construção de textos 2,30 e
hashes 2,89. Dispersão e mudanças dos tempos A0 impedem tratar a comparação
com a rodada anterior como efeito causal isolado. ARC >= A0 continua aberto.

Reprodução: remover GC_STRESS e ARC_CICLOS do ambiente; definir as demais
variáveis conforme acima e executar:

    python scripts/medir-modos-desempenho.py <saída-nova> objetos_escapam textos --repeticoes 7 --afinidade 0x4 --modos A0,ARC --sem-dart

# Perfil da drenagem em árvores e lista — Windows, 2026-10-10

Executável `objetos_escapam-ARC.exe` da matriz `55beeb34`, SHA-256
`56a2136351e329c4880bc737e61c47f4a44c178d9ef7676998b797c2cf5a8a32`.
Afinidade `0x4`; ARC puro, heap 256 MiB, auditoria/stress desligados e
política padrão de ciclos. Uma execução diagnóstica com `DARTFORGE_GC_RASTRO=1`.
Inclui aquecimento e todas as rodadas de **árvores e lista ligada**.
Não substitui a medição sem rastro nem mede uma otimização nova.

Código zero; resultados `3156655` e `499999500000`, iguais à matriz.
382 drenagens acumularam 3.182.096 µs. Processo completo com rastro: 4,814 s.
Não atribuir esses tempos ao benchmark sem rastro ou somente às árvores.

| Trecho do laço | µs acumulados | Fração das drenagens |
| --- | ---: | ---: |
| Zeros iniciais | 1.135.149 | 35,67% |
| Ciclos mais zeros posteriores | 709.408 | 22,29% |
| Ponto fixo de efêmeros | 0 | 0% |
| Mortos e tabelas laterais | 233.193 | 7,33% |

`fases.json` conserva nove fases e quatro trechos do laço. As fases são:
raízes, fotos, jovens das raízes, efêmeros, decisão dos jovens, soltura dos
jovens mortos, candidatura das raízes, laço/ações laterais e memória.
Elas acumulam 3.181.642 µs; a diferença até o total inclui trabalho ao redor
das marcas de tempo. O trecho de ciclos inclui uma segunda drenagem de zeros.

`stderr.txt` é o rastro bruto, `stdout.txt` conserva tempos/resultados,
`execucao.json` registra ambiente/identidade e `hashes.json` confere os quatro
arquivos. Os bytes são preservados por `.gitattributes`.

Inspeção desse trecho identificou uma duplicação evitável de handles mortos:
o heap copiava a lista retornada por `tomar_mortos` para o reclamador, e
depois usava a primeira lista apenas para testar se havia mortos. A mudança
posterior transfere o buffer ao destino vazio e acrescenta rodadas seguintes;
essa execução é o perfil **anterior** à mudança. Ganho de desempenho da
transferência ainda exige comparação controlada própria.

# Transferência de mortos ARC — comparação local, Windows

Antes: executável da revisão `55beeb34b8c57e44cd7563274b396b7550a5d19d`.
Depois: `b706e0b0b073dc61af16aa866ce25bc53397dddd`. A diferença nos fontes
de produção está restrita a `runtime/src/arc.rs` e `runtime/src/heap.rs`:
transferência do buffer de mortos ao reclamador. Fontes do benchmark iguais.

CLI nova recompilada release com `--features jit`, SHA-256
`3a3c71197e2cc6cf1e2434f43d9a76ed1e5d1051f6a4d5a774f4042d6b2dabc3`.
Os dois executáveis usam ARC, sombra/checagem e AOT otimizado. O anterior
permaneceu preservado; o novo foi gerado com `--memoria=arc` explícito.

Sete pares alternados, afinidade Windows `0x4`, rastro/auditoria/stress
desligados, ARC puro, heap 256 MiB e política padrão de ciclos. Quatorze
processos retornaram zero e produziram os mesmos resultados. Hashes dos dois
executáveis e da CLI permaneceram iguais até o fechamento.

| Kernel | Antes, ms | Depois, ms | Depois/antes | Redução observada |
| --- | ---: | ---: | ---: | ---: |
| Árvores | 676,749 | 654,343 | 0,966892 | 3,31% |
| Lista ligada | 129,341 | 124,507 | 0,962626 | 3,74% |

Cada processo executa seis rodadas; a primeira é descartada. A tabela usa
a mediana das cinco restantes, seguida da mediana das sete execuções.
`resultado.json` também conserva as sete razões pareadas: o depois foi
menor em seis pares de árvores e cinco de lista ligada. Houve pares mais
lentos na versão nova; a amostra não estabelece ganho em todos os cenários
nem constitui uma estimativa de significância estatística.

`amostras.jsonl` conserva stdout, stderr, tempos de cada rodada e códigos.
Uma auditoria posterior reconstruiu as medianas do stdout, conferiu as 14
execuções na ordem alternada esperada, os resultados e os hashes. `manifesto.json`
registra fontes, entradas, comando da geração e ambiente. `hashes.json`
identifica oito artefatos arquivados; seus bytes são preservados.

`comparar.py` conserva o comando original e usa os caminhos/revisões da
rodada. Exige os executáveis locais correspondentes e uma saída ainda
inexistente; não sobrescreve os dados da comparação. Não arquivamos binários.

Esta é uma comparação **ARC antes/depois**, com apenas dois kernels. A0
não foi reexecutado e o gate completo não foi reavaliado. O agregado anterior
ARC/A0 = 2,000274 continua reprovado; não extrapolar estas reduções para ele.
ARC permanece exclusivo da seleção explícita `--memoria=arc`.

CI publicada `38057171425`: Windows/Linux/mensagens aprovados; macOS ainda
executava os testes JIT com ignorados ao arquivar esta comparação.

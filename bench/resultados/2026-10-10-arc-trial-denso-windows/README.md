# Trial deletion com índices densos — comparação local, Windows

Antes: `b706e0b0b073dc61af16aa866ce25bc53397dddd`. Depois:
`687a7fd4e062188847c13069eed33f6598751b24`. A diferença de produção
está restrita a `crates/runtime/src/arc.rs`: um mapa handle→índice e
vetores de contadores/marcas substituem três tabelas de hash da região.
Fontes do benchmark iguais nas duas revisões.

CLI release recompilada com `--features jit` em 6 minutos e 5 segundos,
SHA-256 `4daefc7bc235f40bd72a0b0aeabff2f49881f313b8615554c37372e632d7f418`.
O build terminou com sucesso e conserva um aviso de função não usada em
`llvm/raizes.rs::atribuir_slots`. O novo AOT otimizado foi gerado com
`--memoria=arc` explícito, sombra/checagem; o anterior ficou preservado.

Sete pares alternados, afinidade Windows `0x4`, sem rastro, auditoria ou
stress, ARC puro, heap de 256 MiB e política padrão de ciclos. Todos os
14 processos retornaram zero com resultados iguais. Hashes da CLI e
dos dois executáveis foram capturados antes dos pares e conferidos ao final.

| Kernel | Antes, ms | Depois, ms | Depois/antes | Variação observada |
| --- | ---: | ---: | ---: | ---: |
| Árvores | 633,281 | 643,327 | 1,015863 | 1,59% mais lento |
| Lista ligada | 156,701 | 149,509 | 0,954104 | 4,59% mais rápido |

Cada processo executa seis rodadas; descartamos a primeira e calculamos
a mediana das cinco restantes, seguida da mediana das sete execuções.
O depois foi menor em apenas 2/7 pares de árvores e 4/7 de lista ligada.
Há variação entre pares e o resultado é misto: **não demonstra ganho geral**
nem estabelece significância estatística. Não combinar tempos absolutos de
rodadas arquivadas em outros diretórios como se fossem pares desta medição.

`amostras.jsonl` conserva stdout, stderr, códigos e tempos. `auditar.py`
reconstrói as medianas do stdout, confere as 14 posições da alternância,
resultados, fontes e hashes. `manifesto.json` registra os comandos,
revisões e ambiente. `auditoria.json` conserva a conferência concluída.
Os scripts mantêm os caminhos originais e exigem os binários locais
correspondentes; o comparador exige uma saída inexistente. Não arquivamos
binários. `.gitattributes` preserva os bytes dos artefatos.

Logs de correção também preservados: 190 testes unitários release do
runtime, incluindo ignorados, 16 integrações e 42 exemplos públicos.
Os três oráculos de grafos aleatórios passaram com `DF_ARC_SEMENTES=1000`:
3.000 sementes de 400 passos, incluindo ARC normal, ciclos em toda
sincronização e tracing. Regressões novas cobrem handles esparsos,
multiplicidade e aborto no trial negativo antes do descarte.

A0 e os outros 30 kernels não foram reexecutados. O agregado anterior
ARC/A0 = 2,000274 permanece reprovado; esta comparação não reavalia o gate
completo nem conclui os §§27–34. ARC continua exclusivo da seleção
explícita `--memoria=arc`; tracing permanece padrão.

CI `38057171425` terminou aprovada nos três sistemas. A nova rodada
`38064260616`, publicada em `687a7fd4`, segue em execução; o job de
mensagens de commit passou.

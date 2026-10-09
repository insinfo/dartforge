# Junção de faixas livres — medida dirigida

Windows 11, i3-1215U, núcleo P (`0x4`), produção/LTO. Executáveis de
`bench/desempenho/objetos_escapam.dart`: A0 (tracing, pilha-sombra, checagem)
e ARC (pilha-sombra, checagem). Antes: código de `2bb3964a`; depois: a junção
de vizinhos na ponta da lista livre de `EspacoDeObjetos` neste commit.
CLI depois, SHA-256:
`D180A0A18C9A9F304AB9AF77461BC13C037BABA0B5B75539E360C61D1A460274`.

Todos os executáveis foram compilados antes da execução. Duas rodadas
independentes de cinco execuções, alternando `A0-antes`, `A0-depois`,
`ARC-antes`, `ARC-depois` (a ordem gira uma posição por repetição).
Em cada execução, mediana das cinco rodadas após a primeira; depois,
mediana das cinco execuções. Todos os resultados foram iguais nas 40
execuções, com os dois núcleos presentes e código zero.

| núcleo | primeira rodada, antes → depois (ms) | segunda rodada, antes → depois (ms) |
| --- | ---: | ---: |
| A0/árvores | 51,142 → 49,461 | 49,477 → 50,518 |
| A0/lista ligada | 20,924 → 20,452 | 20,832 → 20,831 |
| ARC/árvores | 688,901 → 661,325 | 671,961 → 638,089 |
| ARC/lista ligada | 142,385 → 131,724 | 139,749 → 127,437 |

ARC caiu 4–5% em árvores e 7,5–8,8% em lista ligada. A0 variou −3,3% a
+2,1% em árvores e −2,3% a 0% em lista ligada. Isso confirma um ganho
dirigido, com dispersão; não prova ganho na média do benchmark inteiro.

Os arquivos `amostras-1.jsonl` e `amostras-2.jsonl` preservam stdout, stderr,
código de saída e medianas de cada execução. `resultado-1.txt` e
`resultado-2.txt` contêm o resumo; `executaveis.json` registra os hashes
dos quatro executáveis locais (os binários não estão neste registro).

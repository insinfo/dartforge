# Inicialização parcial de páginas ARC — experiência descartada

Windows 11, i3-1215U, núcleo P (`0x4`), produção/LTO. Antes: executáveis
congelados de `7006187e`; depois: código de `77518978` com alteração local
para inicializar metadados apenas até o último slot registrado. As análises
auxiliares do compilador mudaram entre essas revisões; A0 acompanha a variação.

Dois conjuntos de cinco execuções alternadas de A0/ARC antes/depois.
As 40 execuções terminaram com código zero, mesmos dois núcleos e resultados
iguais. Tempos: mediana das cinco rodadas após a primeira, depois das execuções.

| núcleo | primeira rodada: depois/antes | segunda rodada: depois/antes |
| --- | ---: | ---: |
| A0/árvores | 1,01862 | 1,01154 |
| A0/lista ligada | 1,00912 | 1,01289 |
| ARC/árvores | 0,98942 | 1,17730 |
| ARC/lista ligada | 0,97745 | 0,95834 |

A segunda rodada de árvores tem grande dispersão nos dois executáveis:
medianas por execução antes 552739, 633783, 1083798, 528226, 1004004 µs;
depois 1268738, 746155, 1220153, 549456, 535328 µs. Não há evidência de
melhora consistente em árvores. Lista ligada melhora nestes conjuntos,
mas isso não prova benefício global. A alteração local foi removida.

Os testes release da experiência passaram: 103 testes, três microbenchmarks
ignorados, grafos com 3.000 sementes nos três modos. Isso prova correção
nesses testes, não desempenho. Hashes, stdout, stderr e códigos estão nos
arquivos preservados. O script documenta o protocolo usado; seus caminhos
originais continuam locais ao workspace.

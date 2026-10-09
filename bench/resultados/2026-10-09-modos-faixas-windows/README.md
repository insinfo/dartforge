# Modos nativos depois da junção de faixas — Windows

Código de `7006187e`, Windows 11, i3-1215U, afinidade `0x4` (núcleo P),
Dart 3.6.2 AOT. Mesmo protocolo da [rodada anterior](../2026-10-09-modos-windows/README.md):
32 núcleos de nove programas, seis modos, sete execuções alternadas;
mediana das rodadas sem a primeira, depois das execuções; média geométrica
das razões por núcleo. Os 54 executáveis foram compilados numa pasta nova
antes das medições. CLI SHA-256:
`D180A0A18C9A9F304AB9AF77461BC13C037BABA0B5B75539E360C61D1A460274`.

| razão | antes (rodada anterior) | depois |
| --- | ---: | ---: |
| A1/A0 | 0,983 | 0,991 |
| B0/A0 | 0,959 | 0,969 |
| B1/A0 | 0,950 | 0,952 |
| ARC/A0 | 2,107 | 2,108 |
| A0/Dart AOT | 1,260 | 1,270 |

378 execuções com resultados iguais em todos os modos e repetições, sem
processo falho nem núcleo ausente. A junção de faixas tem ganho dirigido
confirmado em duas rodadas antes/depois de árvores e lista ligada, mas não
reduziu a média completa nesta medida. ARC continua perto de 2,1× A0;
árvores 14,79×, lista ligada 6,93×. As rodadas completas foram feitas em
momentos diferentes: a variação de suas razões não isola o efeito da mudança.

`resultado.md` preserva os tempos por núcleo; `amostras.jsonl` tem configuração,
stdout completo, stderr, código e medianas de cada execução;
`executaveis.json` contém os hashes dos executáveis locais após a rodada.
Os binários não fazem parte deste registro.

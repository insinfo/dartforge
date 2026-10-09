# Modos nativos — Windows, 2026-10-09

Windows 11, i3-1215U, afinidade `0x4` (núcleo P), Dart 3.6.2 AOT. Nove
programas, 32 núcleos, seis modos, sete execuções alternadas: 378 execuções.
Em cada execução, a mediana das rodadas sem a primeira; depois, a mediana
das sete execuções. As razões agregadas usam média geométrica por núcleo.

O código nativo corresponde a `c3dd048b` (inclui `bf0cccbd`, índice ARC sem
divisão); o medidor é o de `3e071d0b`. CLI SHA-256:
`A5C88A0CD4AD9894703251D34C09D9F7C836CBE2FFA294AF32599EDC1EA58930`.
Todos os programas DartForge foram compilados por `aot --optimize`, com os
modos de raízes/exceções/memória definidos em `scripts/medir-modos-desempenho.py`.

Os 54 executáveis foram compilados numa pasta vazia antes das medidas. A
primeira rodada usou o medidor anterior e não preservou os dados brutos:
A1/A0 0,981, B0/A0 0,971, B1/A0 0,958, ARC/A0 2,120, A0/Dart 1,224.
A rodada aqui preservada reutilizou exatamente esses executáveis, sem
recompilar; os hashes foram conferidos antes e depois.

| razão | rodada auditável |
| --- | ---: |
| A1/A0 | 0,983 |
| B0/A0 | 0,959 |
| B1/A0 | 0,950 |
| ARC/A0 | 2,107 |
| A0/Dart AOT | 1,260 |

Todos os resultados foram iguais em todos os modos e repetições; nenhum
processo falhou e nenhum núcleo faltou. A diferença entre as duas rodadas
mostra a dispersão. ARC permanece perto de 2,1× A0; não há evidência aqui
de ganho relevante na média completa pela troca do índice. Árvores ainda
custam 14,65× A0 e lista ligada 7,63×.

* [resultado.md](resultado.md): tempos por núcleo e razões.
* [amostras.jsonl](amostras.jsonl): configuração e cada execução, com stdout
  completo (incluindo aquecimento), stderr, código e mediana por núcleo.
* [executaveis.json](executaveis.json): SHA-256 dos 54 executáveis locais;
  os binários não fazem parte deste registro.

Com SDK e CLI preparados pelo `scripts/env.ps1`, numa pasta de saída nova:

```powershell
. ./scripts/env.ps1
$env:DARTFORGE_DART_SDK = $env:DART_SDK
python scripts/medir-modos-desempenho.py target/tmp-modos --repeticoes 7 --afinidade 0x4
```

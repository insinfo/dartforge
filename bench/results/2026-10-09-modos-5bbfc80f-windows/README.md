# ARC contra A0 — Windows, fontes de 5bbfc80f

Windows 11, i3-1215U, afinidade `0x4` (núcleo P). Nove programas,
32 kernels, A0 e ARC, sete execuções alternadas: 126 processos válidos.
Todos os resultados foram iguais entre modos/repetições e aos resultados
Dart AOT da rodada `2026-10-09-modos-faixas-windows`; os fontes de
`bench/desempenho` permanecem iguais. O Dart não foi reexecutado nesta rodada.

CLI recompilada em release com `--features jit` (LLVM embutido), SHA-256
`3F9B8C99A1131044E9E12B569ECB32B058879F0384F5B103F9DFC69ADBA477CD`.
Os 18 executáveis foram compilados por `aot --optimize` numa pasta nova.
A0: sombra/checagem/tracing; ARC: sombra/checagem/ARC puro, política padrão
de ciclos. Limite de heap 256 MiB; auditoria, rastro e GC off desativados.
`DARTFORGE_GC_STRESS` ausente. Hashes capturados na primeira repetição e
conferidos após as sete: nenhum executável mudou.

ARC/A0 foi **1,968695** na média geométrica dos 32 kernels. Árvores 13,25×,
construção de textos 8,31× e lista ligada 5,27×. ARC ≥ A0 continua pendente.
As várias mudanças desde a rodada anterior e a dispersão impedem atribuir
a diferença agregada ao corte de raízes isoladamente.

Uma tentativa inicial definiu `DARTFORGE_GC_STRESS=0`. O runtime interpreta
a presença dessa variável como ativação de estresse. Essa tentativa foi
encerrada; `amostras-estresse-invalida.jsonl` conserva seus dados parciais,
excluídos de todas as razões. A rodada corrigida reutilizou os mesmos 18
executáveis após conferir hashes, removendo a variável.

`amostras.jsonl` preserva as 126 execuções, stdout completo, aquecimento,
stderr e códigos. `resultado.md` contém medianas e razões; A1/B0/B1 não
foram medidos. `executaveis.json` e `cli.json` registram os hashes, e
`auditoria.json` o fechamento independente de cobertura, igualdade e razão.
Os binários permanecem locais em `target/modos-2026-10-09-5bbfc80f-a0-arc`.

Reprodução, com SDK/LLVM preparados:

```powershell
. ./scripts/env.ps1
Remove-Item Env:DARTFORGE_GC_STRESS -ErrorAction SilentlyContinue
Remove-Item Env:DARTFORGE_ARC_CICLOS -ErrorAction SilentlyContinue
$env:DARTFORGE_GC_RASTRO = '0'
$env:DARTFORGE_GC_OFF = '0'
$env:DARTFORGE_ARC_CONFERIR = '0'
$env:DARTFORGE_ARC_BERCARIO = '0'
$env:DARTFORGE_HEAP_MAX_MB = '256'
$env:PYTHONIOENCODING = 'utf-8'
python scripts/medir-modos-desempenho.py target/modos-novos --repeticoes 7 --afinidade 0x4 --modos A0,ARC --sem-dart
```

Começar com pasta nova para recompilar todos os executáveis. A mediana de
cada execução descarta a primeira rodada de cada kernel; usa-se depois a
mediana das sete execuções e a média geométrica das razões por kernel.

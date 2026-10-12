# ARC contra A0 — Windows, fontes de 55beeb34

Revisão `55beeb34b8c57e44cd7563274b396b7550a5d19d`, sem alterações locais
nos fontes. CLI release recompilada com `--features jit` (LLVM embutido),
SHA-256 `2A63E6782B507291A3277FBCB00B0307DEA0CDB10AB7184BE33125E8FB7239B5`.
SDK Dart 3.6.2. Nove programas, 32 kernels, sete execuções alternadas A0/ARC,
afinidade Windows `0x4`: 126 processos concluídos com código zero.

Média geométrica ARC/A0: **2,000274042309175**. O gate ARC≥A0 permanece
**reprovado**. Árvores 12,82×, lista ligada 5,28× e mapa de strings para
objetos 4,88× são os maiores custos relativos. `resultado.md` conserva todas
as medianas; `auditoria.json` traz razões sem arredondamento.

Todos os resultados foram iguais entre modos/repetições e aos resultados
Dart AOT arquivados em `2026-10-09-modos-faixas-windows`. Dart não foi
reexecutado. Os bytes das dez fontes do benchmark foram conferidos contra
`5bbfc80f`; não mudaram. O agregado anterior 1,968695 pertence a essa revisão
anterior. Mudanças acumuladas e dispersão não permitem atribuir a diferença
à instrumentação condicional das drenagens isoladamente.

Executáveis gerados por `aot --optimize` em pasta nova. A0 usa sombra,
checagem e tracing; ARC usa sombra, checagem e ARC puro. Ciclos seguem a
política padrão; heap de 256 MiB. Auditoria, rastro e GC off desativados;
`DARTFORGE_GC_STRESS` e `DARTFORGE_ARC_CICLOS` ausentes. ARC continua opt-in
pelo parâmetro de memória explícito; tracing permanece o padrão da CLI.

`amostras.jsonl` conserva stdout completo, aquecimento, stderr, código e
medianas de cada execução. A auditoria independente reconstrói a mediana
descartando o aquecimento, verifica cobertura, ordem alternada e paridade
dos resultados, e recalcula o agregado. A captura dos 18 hashes ocorreu no
início da fase de execução, após duas execuções concluídas; ao fechar a
rodada, todos permaneciam iguais. Esse ponto está explícito em
`proveniencia.json`. Não se afirma captura anterior à primeira execução.

`hashes.json` identifica os oito artefatos arquivados; `.gitattributes`
preserva seus bytes. Executáveis ficam locais em
`target/modos-2026-10-10-55beeb34-a0-arc`. Build e execução têm logs brutos.
O aviso de `atribuir_slots` não usado é preexistente.

Para reproduzir, prepare SDK/LLVM, recompile a CLI e use uma pasta nova:

```powershell
. ./scripts/env.ps1
cargo build --locked --release -p dartforge-cli --features jit
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

O auditor arquivado usa os caminhos da rodada original e seus hashes iniciais.
Não certifica os §§27–34 completos, vidas/observadores ou políticas de memória.
Ao fechar a rodada, a CI publicada `38057171425` tinha Windows, Linux e
mensagens aprovados; macOS ainda executava os testes emit_native com ignorados.

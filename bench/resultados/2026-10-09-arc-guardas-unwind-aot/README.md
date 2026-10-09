# Retorno Guarda e unwind AOT entre funções Dart

Prova dirigida sobre `351c99d7` com a separação automática de saídas Guarda
na transação ARC do conjunto. Caller com invoke/pouso preparados; callee
com retorno Ref borrowed e tabela Guarda. Nenhuma classe ou efeito manual.

Para os modos `arc`/`tracing` e casos `normal`/`erro`:

```powershell
cargo run --locked -p dartforge-emit-native --example arc_retornos_guardados -- target/prova-guarda-arc-normal.exe arc normal
$env:DARTFORGE_ARC_CONFERIR='1'
$env:DARTFORGE_ARC_BERCARIO='0'
$env:DARTFORGE_GC_STRESS='1'
& target/prova-guarda-arc-normal.exe
```

As quatro execuções terminaram com código 0. Normal: uma linha
`9223372036854775807`. Erro: saída vazia. No normal, o caller libera o
resultado Owned retido pelo callee, coleta e ainda observa o Mint pelo
token independente da fábrica. No erro, o callee lança o Ref emprestado,
desenrola por `df.lancar` e o caller trata pelo landingpad do invoke,
limpa a exceção e libera o token da fábrica antes de retornar.

O passe exige exatamente uma retenção e três drops inseridos nos caminhos
possíveis; não significa que todos esses caminhos rodaram em cada processo.
Os quatro LLVM incluem invoke do callee, landingpad e desenrolamento.
IR e stdout preservam bytes; hashes locais estão em `evidencias.json`.
Executáveis permanecem em `target`.

Os testes HIR cobrem Owned transferido só no sucesso, retenção de borrowed
só no sucesso, transporte de fechamento léxico para ambas as saídas,
idempotência e atomicidade em erro de escopo/esgotamento de IDs.
Não observa morte final no AOT nem certifica ausência geral de vazamentos.
Não gera automaticamente os invokes dos callers, integra no pipeline padrão,
resolve arestas críticas gerais, finally, cancelamento ou suspensão.

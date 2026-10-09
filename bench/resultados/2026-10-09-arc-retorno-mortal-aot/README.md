# Retorno retido com Mint mortal — Windows, 2026-10-09

A variante `retorno-mortal` cria i64::MAX pela fábrica de Ref Owned. A
função identidade recebe esse valor emprestado e seu retorno ganha uma
ArcCopy pelo passe. O token devolvido vai ao slot forte; o token original
da fábrica é liberado antes de fechar o quadro e coletar. O valor carregado
do global é observado depois dessa coleta.

ARC com auditoria/berçário desligado/estresse e tracing com estresse saíram
zero e imprimiram `9223372036854775807`, `42`, `null`. A variante sem
cleanup saiu 1 por token v8 restante, sem emitir IR ou executável negativo.
O LLVM contém a fábrica Owned, chamada real ao callee, retain do retorno
e release do token original. Hashes e flags estão em `resultados.json`.

```powershell
cargo run --locked -p dartforge-emit-native --example arc_slots_fortes -- target/prova-mortal-arc.exe arc retorno-mortal
cargo run --locked -p dartforge-emit-native --example arc_slots_fortes -- target/prova-mortal-tracing.exe tracing retorno-mortal
$env:DARTFORGE_ARC_CONFERIR = '1'
$env:DARTFORGE_ARC_BERCARIO = '0'
$env:DARTFORGE_GC_STRESS = '1'
& target/prova-mortal-arc.exe
& target/prova-mortal-tracing.exe
cargo run --locked -p dartforge-emit-native --example arc_slots_fortes -- target/prova-mortal-negativo.exe arc retorno-mortal-sem-cleanup
```

Confirma a sobrevivência do valor mortal durante o uso; não observa sua
morte após o último owner no AOT. O teste runtime da fábrica cobre essa
morte separadamente. Não prova ausência de todos os vazamentos, erro real
de impressão, inserção automática de drops/cleanup, suspensão, Finalizable,
desempenho ou integração no lowering padrão. O contrato do callee continua
explícito no fixture, sustentado pelo corpo preparado/verificado.

# Retenção inserida em retorno Dart — Windows, 2026-10-09

A variante `retorno` do exemplo de slots chama uma função identidade Ref.
Seu corpo inicial apenas retorna o parâmetro; o passe insere uma ArcCopy e
verifica a função completa. O LLVM congelado contém a chamada real e um
`dartforge_arc_retain` no callee, antes de devolver seu resultado Owned.
O contrato dessa chamada no fixture é explícito, sustentado pela função
preparada/verificada; não se presume contrato para callees arbitrários.

ARC com auditoria/berçário desligado/estresse e tracing com estresse saíram
zero e imprimiram os mesmos três valores. A variante sem cleanup saiu 1
por token v8 restante, antes de emitir IR ou executável. Hashes, flags e
base da alteração estão em `resultados.json`; executáveis ficam em target.

```powershell
cargo run --locked -p dartforge-emit-native --example arc_slots_fortes -- target/prova-retorno-arc.exe arc retorno
cargo run --locked -p dartforge-emit-native --example arc_slots_fortes -- target/prova-retorno-tracing.exe tracing retorno
$env:DARTFORGE_ARC_CONFERIR = '1'
$env:DARTFORGE_ARC_BERCARIO = '0'
$env:DARTFORGE_GC_STRESS = '1'
& target/prova-retorno-arc.exe
& target/prova-retorno-tracing.exe
cargo run --locked -p dartforge-emit-native --example arc_slots_fortes -- target/prova-retorno-negativo.exe arc retorno-sem-cleanup
```

O valor é um literal permanente: esta prova não demonstra morte de objeto
ou contagem física sobre objeto mortal. Não exerce erro real de impressão,
inserção de drops/cleanup, suspensão, Finalizable, desempenho ou integração
no lowering padrão de programas Dart. A variante normal do exemplo e os
arquivos anteriores de slots permanecem disponíveis.

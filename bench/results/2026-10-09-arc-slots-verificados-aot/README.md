# Slots fortes com planos verificados — Windows, 2026-10-09

O exemplo `arc_slots_fortes.rs` agora produz e verifica classes, contratos
runtime e mapa de pendências no CFG final, antes da emissão LLVM. Apenas o
literal permanente e a caixa de 42 (Smi imediato) têm contratos Trivial
específicos fornecidos pelo harness. As impressões têm leitura real de
pendência e cleanup do token carregado e do global no caminho de erro.

ARC com auditoria/berçário desligado/estresse e tracing com estresse executaram
com código zero e saída idêntica: `slots ARC`, `42`, `null`. O driver otimiza
ambas as imagens. A variante `sem-cleanup` foi recusada pelo verificador com
token v8 não consumido no caminho [0, 1], código 1, antes de gravar IR.
`resultados.json` registra hashes e flags explícitas, sem afirmar inventário
completo do ambiente. Arquivos congelados como bytes; executáveis ficam em target.

```powershell
cargo run --locked -p dartforge-emit-native --example arc_slots_fortes -- target/prova-slots-verificados-arc.exe arc
cargo run --locked -p dartforge-emit-native --example arc_slots_fortes -- target/prova-slots-verificados-tracing.exe tracing
$env:DARTFORGE_ARC_CONFERIR = '1'
$env:DARTFORGE_ARC_BERCARIO = '0'
$env:DARTFORGE_GC_STRESS = '1'
& target/prova-slots-verificados-arc.exe
& target/prova-slots-verificados-tracing.exe
cargo run --locked -p dartforge-emit-native --example arc_slots_fortes -- target/prova-slots-verificados-sem-cleanup.exe arc sem-cleanup
```

Essa prova não exercita erro real de impressão: a recusa é estrutural no CFG.
Não prova coleta mortal, proveniência/vida de slots, inserção automática de
cleanup, parâmetros gerais, Finalizable, desempenho ou integração do produtor
no pipeline padrão de programas Dart. O arquivo anterior de slots fortes
permanece como evidência da versão sem planos verificados.

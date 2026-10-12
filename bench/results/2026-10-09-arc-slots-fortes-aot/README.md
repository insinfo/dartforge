# Slots fortes em AOT — Windows, 2026-10-09

Fonte: `7e5be7a8`, exemplo `crates/emit_native/examples/arc_slots_fortes.rs`.
HIR explícita, otimização habilitada, driver e runtime reais. Duas imagens
executadas com código zero e saída idêntica: `slots ARC`, `42`, `null`.
Os arquivos `.ll` são o IR entregue ao driver; os executáveis ficam em
`target/arc-slots-compilados`, com hashes registrados, sem entrar no histórico.

ARC executou com `DARTFORGE_ARC_CONFERIR=1`, `DARTFORGE_ARC_BERCARIO=0`
e `DARTFORGE_GC_STRESS=1`. Tracing executou com `DARTFORGE_GC_STRESS=1`.
`resultados.json` registra somente essas flags explícitas, sem afirmar um
inventário completo do ambiente. Os arquivos são preservados como bytes.

A prova exercita transferência SSA→quadro→global, carga após fechar o quadro,
coleta explícita, substituição por Smi/null e leitura do armazenamento real.
O harness declara `dartforge_print_handle` como auxiliar de observação. O
literal é permanente: não comprova coleta de objeto mortal, inserção automática,
escopos/borrows, inicialização lazy, ABI importada, desempenho ou o ARC completo.

Reprodução na raiz do repositório, com a configuração LLVM do Cargo:

```powershell
cargo run --locked --release -p dartforge-emit-native --example arc_slots_fortes -- target/arc-slots-compilados/arc.exe arc
cargo run --locked --release -p dartforge-emit-native --example arc_slots_fortes -- target/arc-slots-compilados/tracing.exe tracing
$env:DARTFORGE_ARC_CONFERIR = '1'
$env:DARTFORGE_ARC_BERCARIO = '0'
$env:DARTFORGE_GC_STRESS = '1'
& target/arc-slots-compilados/arc.exe
& target/arc-slots-compilados/tracing.exe
```

As flags de conferência/berçário só afetam ARC. A primeira tentativa sem a
declaração do auxiliar foi recusada pelo Clang e corrigida antes desta prova;
executar o gerador diretamente, sem o ambiente aplicado pelo Cargo, não
encontrou clang.exe. Esses erros exploratórios não são execuções aprovadas.

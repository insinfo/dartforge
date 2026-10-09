# Alvo mortal em global forte — AOT Windows, 2026-10-09

Fonte `93abd434`, exemplo `arc_slots_mortais`. HIR explícita otimizada,
driver/runtime reais. ARC e tracing: código zero, saída `1`, `null`.
A função produtora retorna antes da coleta. O observador LLVM lê uma
referência fraca, sem registrar o alvo como raiz forte no chamador.
O contêiner weak permanece protegido por um token independente.

Controles negativos ARC, com auditoria, berçário desativado e estresse:
`sem-owner` aborta na primeira conferência, sem saída; `sem-liberar`
imprime `1` e aborta na segunda. Ambos com trap 0xC000001D esperado.
As execuções negativas não são programas aprovados pelo corpus.

Reprodução na raiz, usando o ambiente LLVM configurado pelo Cargo:

```powershell
cargo run --locked --release -p dartforge-emit-native --example arc_slots_mortais -- target/arc-mortal/arc.exe arc
cargo run --locked --release -p dartforge-emit-native --example arc_slots_mortais -- target/arc-mortal/tracing.exe tracing
cargo run --locked --release -p dartforge-emit-native --example arc_slots_mortais -- target/arc-mortal/sem-owner.exe arc sem-owner
cargo run --locked --release -p dartforge-emit-native --example arc_slots_mortais -- target/arc-mortal/sem-liberar.exe arc sem-liberar
$env:DARTFORGE_ARC_CONFERIR='1'
$env:DARTFORGE_ARC_BERCARIO='0'
$env:DARTFORGE_GC_STRESS='1'
& target/arc-mortal/arc.exe
& target/arc-mortal/tracing.exe
& target/arc-mortal/sem-owner.exe
& target/arc-mortal/sem-liberar.exe
```

IR e saídas são preservados como bytes; `resultados.json` registra códigos,
hashes e flags explicitamente aplicadas, não um inventário completo do ambiente.
Executáveis permanecem em `target`, fora do histórico. As flags ARC são
irrelevantes em tracing. Arquivos `.ll` são o IR entregue ao driver.

Esta prova não certifica inserção automática, promoção weak com retenção,
Finalizable, ABI importada ou desempenho. O alvo é um bloco mortal bruto,
sem grafo de filhos; não é uma prova de ciclos nem de instância Dart tipada.

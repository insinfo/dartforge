# Cleanup automático de saídas no AOT

Prova dirigida sobre `8878e784`, com a variante
`retorno-mortal-auto-cleanup` acrescentada ao exemplo `arc_slots_fortes`.

Gerar para cada modo (`arc` e `tracing`):

```powershell
cargo run --locked -p dartforge-emit-native --example arc_slots_fortes -- target/prova-auto-cleanup-arc.exe arc retorno-mortal-auto-cleanup
$env:DARTFORGE_ARC_CONFERIR='1'
$env:DARTFORGE_ARC_BERCARIO='0'
$env:DARTFORGE_GC_STRESS='1'
& target/prova-auto-cleanup-arc.exe
```

Ambos os executáveis terminaram com código 0 e imprimiram
`9223372036854775807`, `42` e `null`. O passe inseriu zero cópias e três
liberações, conferidas pelo próprio exemplo. Os blocos LLVM b1, b3 e b5
liberam respectivamente v8, v14 e v18 antes do retorno, após a limpeza
explícita do global. Os arquivos LLVM e stdout preservam os bytes locais.

O controle `retorno-mortal-sem-cleanup` terminou com código 1 por token v8
não consumido no caminho [0, 1], sem gerar LLVM nem executável.

Esta execução não induz erro de impressão: verifica a inserção nos três
caminhos de erro pelo IR e pelo verificador, e executa o caminho normal.
Não observa a morte final do Mint no AOT nem certifica ausência geral de
vazamentos, integração no lowering padrão, finally, cancelamento ou suspensão.

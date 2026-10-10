# Unwind estrangeiro C++ com heap real — Linux

Fonte `f460ce72e3c570bfb9fe4cd192141ede8937d966`,
[CI 38019611633](https://github.com/insinfo/dartforge/actions/runs/38019611633),
job 114117516525. A etapa de unwind estrangeiro terminou com sucesso; o workflow
completo ainda estava ativo quando esta evidência foi capturada.

Oito execuções: simples/quadros × LLVM O0/O2 × ARC/tracing. Todos os stdout
contêm exatamente `foreign-cleanup-ok` seguido de LF. O script usa os nomes
que o runtime efetivamente lê: DARTFORGE_ARC_CONFERIR=1,
DARTFORGE_ARC_BERCARIO=0 e DARTFORGE_GC_STRESS=1. Geração com
DARTFORGE_EFEITOS=conferir. Esta rodada corrige a lacuna de ativação das opções
nas provas anteriores que usavam nomes sem prefixo.

A fixture guarda o endereço do Mint antes de lançar a exceção C++. Confere
identidade da exceção nativa, destrutor local, desvio do catch Dart, restauração
do topo de raízes, pendência Dart zero, consumo do owner antes da coleta e
morte do objeto após coleta. O perfil quadros guarda o mesmo objeto em dois
quadros locais, usa Copy/Move e carrega um owner independente; a morte final
confirma que o cleanup não deixou esses slots fortes como raízes.

Os quatro IR, quatro fixtures C++ e oito stdout são preservados byte a byte;
SHA-256, fonte, etapa e digest Actions estão em evidencia.json. Não contém
executáveis nem IR otimizado. A substituição explícita do símbolo print_handle
injeta a exceção C++ após emitir HIR com contrato Borrow auditado. Não prova
lowering FFI completo, forced unwind, finally, cancelamento, SEH/statepoints,
inlining observado ou ganho de desempenho. Não contém o perfil Retoma com
quadros, desenvolvido posteriormente. macOS não é coberto por este arquivo.

Reprodução na fonte indicada, em Linux:

```sh
DARTFORGE_EFEITOS=conferir cargo run --locked --release -p dartforge-emit-native \
  --features llvm-embutido --example arc_unwind_estrangeiro -- target/prova 0 quadros
DARTFORGE_ARC_CONFERIR=1 DARTFORGE_ARC_BERCARIO=0 DARTFORGE_GC_STRESS=1 \
  target/prova arc
```

Variar 0/2, simples/quadros e arc/tracing para as oito combinações.

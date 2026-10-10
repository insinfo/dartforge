# Boxing Owned e cleanup estrangeiro — Linux e macOS

Fonte `f81c7d2b2d2243fcf0eedf53bfc2a210dfca26b7`,
[CI 38025125587](https://github.com/insinfo/dartforge/actions/runs/38025125587).
Workflow concluído com sucesso, incluindo Windows, Linux, macOS e mensagens.

A fixture parte de `Box` HIR de `i64::MAX`. A preparação ARC o transforma
na fábrica `dartforge_arc_box_int_owned_v1`, cujo resultado já possui o token
Owned. Os 12 IR preservados contêm a chamada dessa fábrica com o valor esperado.
O cleanup consome esse token ao propagar o unwind estrangeiro ou forçado.

São 72 execuções: Linux/macOS × simples/quadros/retoma × LLVM O0/O2 ×
ARC/tracing × C++/forçado/forçado com classe Dart. Os 24 stdout C++ contêm
exatamente `foreign-cleanup-ok` seguido de LF; os 48 forçados contêm
`forced-cleanup-ok` seguido de LF. Foram conferidos todos os bytes.
`evidencia.json` registra SHA-256 de 114 arquivos, fonte, jobs, etapas e
digests dos artefatos Actions. `.gitattributes` preserva os bytes originais.

Geração usa `DARTFORGE_EFEITOS=conferir`; execução usa
`DARTFORGE_ARC_CONFERIR=1`, `DARTFORGE_ARC_BERCARIO=0` e
`DARTFORGE_GC_STRESS=1`. A fixture verifica destrutores, restauração das raízes,
pendência Dart zero, consumo do token e morte do Mint após coleta. Quadros
usam dois slots fortes e Copy/Move; Retoma os fecha em LIFO. C++ verifica a
identidade da exceção e seu destrutor. O unwind forçado verifica objeto,
classe e flags até END_OF_STACK, sem catch Dart ou C++.

C++ pertence somente à fixture de interoperabilidade; compilador e runtime
permanecem Rust. No macOS o runtime fica numa dylib separada para respeitar
o limite de personalidades compact unwind por imagem. Os comandos de ligação
do executável e da dylib estão preservados.

O forçado com classe Dart usa um cabeçalho com tag DARTFRGE, sem payload ou
pendência Dart. Não comprova unwind Dart real, finally/cancelamento,
SEH/statepoints, lowering FFI completo, integração de ownership no fluxo
normal nem desempenho. As mudanças locais posteriores a esta fonte,
incluindo a ABI de leitura escalar de campos, não são cobertas pela execução.
ARC na CLI continua exclusivo de `--memoria=arc`; tracing é o padrão.

Reprodução na fonte indicada, em Unix:

```sh
DARTFORGE_EFEITOS=conferir cargo run --locked --release -p dartforge-emit-native \
  --features llvm-embutido --example arc_unwind_estrangeiro -- target/prova 0 retoma
DARTFORGE_ARC_CONFERIR=1 DARTFORGE_ARC_BERCARIO=0 DARTFORGE_GC_STRESS=1 \
  target/prova arc forcado_dart
```

Variar 0/2, simples/quadros/retoma, arc/tracing e cpp/forcado/forcado_dart.

# Cleanup estrangeiro e unwind forçado — Linux e macOS

Fonte `34198a5435fb0ca243907e55c3b3598919834b90`,
[CI 38021212710](https://github.com/insinfo/dartforge/actions/runs/38021212710).
As etapas de unwind passaram nos dois sistemas; o job Linux inteiro passou.
Windows e o restante do job macOS ainda estavam ativos na captura.

São 72 execuções: Linux/macOS × simples/quadros/retoma × LLVM O0/O2 ×
ARC/tracing × C++/forçado/forçado com classe Dart. Os 24 stdout C++ contêm
exatamente `foreign-cleanup-ok` seguido de LF; os 48 forçados contêm
`forced-cleanup-ok` seguido de LF. Todos os bytes foram conferidos.
IR, fixtures C++, comandos de ligação e stdout são preservados sem alteração;
`evidencia.json` registra SHA-256 de 114 arquivos, etapas, fonte e digests Actions.

Geração usa `DARTFORGE_EFEITOS=conferir`; execução usa
`DARTFORGE_ARC_CONFERIR=1`, `DARTFORGE_ARC_BERCARIO=0` e
`DARTFORGE_GC_STRESS=1`, nomes reconhecidos pelo runtime.

A fixture confere destrutor local, restauração das raízes, pendência Dart zero,
consumo do token e morte do Mint após coleta. C++ também confere identidade
da exceção e seu destrutor. O stop callback forçado confere identidade do
objeto, classe e flags FORCE_UNWIND/CLEANUP_PHASE e só encerra após END_OF_STACK.
Quadros locais usam dois slots fortes e Copy/Move; Retoma fecha ambos em LIFO
antes de propagar o objeto original. O forçado não executa catch Dart nem C++.

No macOS, o runtime fica numa dylib separada para respeitar o limite de
personalidades compact unwind por imagem. Os comandos de ambas as ligações
estão no arquivo; não foi desabilitada a informação de unwind.

O objeto forçado com classe Dart é uma injeção de falha com o tag DARTFRGE,
sem payload ou pendência Dart. Não prova a identidade nem a destruição de
um objeto de unwind Dart real. Também não prova finally/cancelamento,
SEH/statepoints, lowering FFI completo, inlining observado, integração ARC
ao pipeline padrão ou desempenho. Esta fonte precede a preparação automática
de Box escalar do commit a91d9cfd.

Reprodução na fonte indicada, em Unix:

```sh
DARTFORGE_EFEITOS=conferir cargo run --locked --release -p dartforge-emit-native \
  --features llvm-embutido --example arc_unwind_estrangeiro -- target/prova 0 retoma
DARTFORGE_ARC_CONFERIR=1 DARTFORGE_ARC_BERCARIO=0 DARTFORGE_GC_STRESS=1 \
  target/prova arc forcado_dart
```

Variar 0/2, simples/quadros/retoma, arc/tracing e cpp/forcado/forcado_dart.

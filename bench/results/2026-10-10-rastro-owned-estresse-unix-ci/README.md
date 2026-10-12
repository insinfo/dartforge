# Captura Owned da exceção e do rastro sob estresse — Linux e macOS

Fonte `f460ce72e3c570bfb9fe4cd192141ede8937d966`,
[CI 38019611633](https://github.com/insinfo/dartforge/actions/runs/38019611633).
As etapas AOT de retomada dos dois sistemas terminaram com sucesso. Linux
completo já estava aprovado; macOS e workflow ainda ativos na captura.

Dezesseis execuções nativas O2: simples/misto × ARC/tracing × normal/erro ×
Linux/macOS. Todas imprimiram exatamente MAX seguido de LF. Os dezesseis IR
usam lançamento com rastro explícito e getter Owned do rastro; no erro,
capturam exceção e rastro como Owned, comparam identidades, limpam pendência,
liberam originais e coletam. Retenção adicional do rastro após coleta e impressão
da exceção capturada continuam válidas. O propagador retoma o par do pouso;
o perfil misto combina catch preparado e invoke automático.

Geração DARTFORGE_EFEITOS=conferir; execução DARTFORGE_ARC_CONFERIR=1,
DARTFORGE_ARC_BERCARIO=0 e DARTFORGE_GC_STRESS=1. São os nomes que o runtime
consome, corrigindo a ativação ausente nas rodadas anteriores que usavam nomes
sem prefixo. Esses arquivos históricos permanecem intactos; esta é evidência
nova. Não é uma medição de desempenho nem comprova o gate ARC ≥ A0.

Os 32 arquivos IR/stdout são preservados sem conversão de bytes; hashes,
fonte, jobs, etapas e digests Actions constam em evidencia.json. Executáveis e
IR otimizado não foram publicados. Não prova identidade do objeto nativo de
unwind Dart, morte final após soltar todas as capturas, forced unwind, finally,
inlining observado, cancelamento, SEH/statepoints ou integração automática de
ownership no pipeline do programa.

Reprodução na fonte indicada, em cada sistema-alvo:

```sh
DARTFORGE_EFEITOS=conferir cargo run --locked --release -p dartforge-emit-native \
  --features llvm-embutido --example arc_retornos_guardados \
  -- target/prova arc erro automatico misto
DARTFORGE_ARC_CONFERIR=1 DARTFORGE_ARC_BERCARIO=0 DARTFORGE_GC_STRESS=1 target/prova
```

Variar arc/tracing, normal/erro e simples/misto para as oito combinações.

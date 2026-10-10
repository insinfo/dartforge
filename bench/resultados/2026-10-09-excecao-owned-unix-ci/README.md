# Captura Owned da exceção — AOT Linux e macOS

Fonte `7418650dbb584f22e186fe18b9c1f79874bbe7d4`,
[CI 38013924479](https://github.com/insinfo/dartforge/actions/runs/38013924479).
Os passos AOT dos dois sistemas terminaram com sucesso. O JSON registra
separadamente o estado observado de cada job e do workflow completo.

Dezesseis execuções: simples/misto × ARC/tracing × normal/erro × Linux/macOS.
Todos os casos saíram com código zero e imprimiram MAX seguido de LF,
conferido byte a byte. Geração LLVM embutido O2 com DARTFORGE_EFEITOS=conferir;
execução ARC_CONFERIR=1, BER=0, GC_STRESS=1.

No erro, o catch captura a exceção como Ref Owned, compara identidade com o
owner original, limpa a pendência, libera o original, coleta e imprime a
captura. O propagador Unix faz cleanup/resume do par recebido. O perfil misto
combina catch preparado e invoke automático na mesma função.

Os 32 arquivos IR/stdout foram preservados sem conversão de bytes, com hashes,
fonte, jobs e digests dos artefatos Actions no JSON. O IR é entrada do LLVM;
executáveis e IR otimizado não foram publicados pelo workflow.

Reprodução na fonte indicada e no sistema-alvo:

```sh
DARTFORGE_EFEITOS=conferir cargo run --locked --release -p dartforge-emit-native \
  --features llvm-embutido --example arc_retornos_guardados \
  -- target/prova arc erro automatico misto
ARC_CONFERIR=1 BER=0 GC_STRESS=1 target/prova
```

Trocar simples/misto, arc/tracing e normal/erro para os outros casos.
Esta fonte antecede a captura Owned do StackTrace e sua comparação AOT.
Não prova identidade do objeto nativo de unwind, morte final em AOT, inlining,
unwind estrangeiro com owners do catch, finally/cancelamento, SEH/statepoints,
lowering completo, §§27–34 completos nem o gate de desempenho ARC/A0.

# Callback de NativeFinalizer

Fonte `0f6629ad`. CLI recompilada com feature jit e LLVM embutido; hash
em cli.json. Onze subprocessos terminaram com código zero: Dart válido,
quatro compilações AOT, quatro execuções AOT e duas execuções JIT.

`diagnostico.dart` anexa callbacks Pointer.fromFunction e
NativeCallable.isolateLocal. AOT tracing, AOT ARC e JIT retornaram
`[true, true]`: ArgumentError antes da publicação do anexo. Este uso é
explicitamente não suportado pelo contrato do SDK; não foi executado como
oráculo Dart. O teste unitário cobre também a classificação do ouvinte e
a remoção do registro, mas esta rodada não exercita callback listener.

O caso válido `corpus/nativo/14_finalizadores.dart` usa funções C reais e
asTypedList(finalizer:). Os três caminhos produziram a mesma saída que
Dart 3.6.2: `[false, false, true]`, `1225`, `[9, true]`.

Execuções nativas com GC_STRESS presente (1), heap 256 MiB, ARC_CONFERIR=1,
ARC_CICLOS=sempre, GC_RASTRO=0. AOT --optimize com memória tracing/arc;
JIT usa o modo padrão da CLI run, sem afirmar cobertura ARC no JIT.
SDK_DLL removida para usar o cache da revisão corrente. Não é validação
do corpus completo, desempenho ou encerramento por grupo de isolados.

Reprodução após compilar a CLI desta fonte e carregar scripts/env.ps1:

    python bench/resultados/2026-10-09-finalizador-callback/validar.py <diretório-novo>

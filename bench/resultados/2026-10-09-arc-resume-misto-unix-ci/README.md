# ARC resume simples/misto — AOT Linux e macOS

Fonte `5445efd54f24100d4cf3b1cd821de340bd115d85`,
[CI 38011679848](https://github.com/insinfo/dartforge/actions/runs/38011679848).
Passos AOT completados com sucesso em Linux x86-64 e macOS ARM64;
workflow inteiro ainda ativo na conferência. Dezesseis execuções reais:
simples/misto × ARC/tracing × normal/erro × dois sistemas, LLVM embutido O2,
`ARC_CONFERIR=1 BER=0 GC_STRESS=1`. Todos saíram com código zero.

Stdout byte exato: normal MAX seguido de LF; erro vazio. No perfil misto,
o caller tem catch explícito e invoke automático de propagação, usando a
personalidade nova. Artefatos confirmam teste de seletor com
llvm.eh.typeid.for(null), caminho estrangeira com resume e cleanup automático.
No erro Dart, o propagador faz cleanup/resume e o caller executa seu catch.
A prova usa Mint real e guarda sua raiz de pendência; não executa estrangeira.

Trinta e dois arquivos IR/stdout copiados sem conversão de bytes. JSON registra
hashes, jobs, fonte e digests dos artefatos Actions. IR é entrada, não IR
LLVM otimizado; executáveis não foram publicados pelo workflow. Não há
observação direta de inlining, identidade/rastro nativo ou morte final.

Reproduzir no sistema-alvo com a toolchain do repositório:

```sh
cargo run --locked --release -p dartforge-emit-native --features llvm-embutido \
  --example arc_retornos_guardados -- target/prova-resume arc erro automatico misto
ARC_CONFERIR=1 BER=0 GC_STRESS=1 target/prova-resume
```

Omitir misto para o perfil simples; trocar arc/tracing e normal/erro para os
outros casos. A leitura Owned da exceção foi acrescentada depois desta fonte;
ela muda o stdout de erro e não está nestes arquivos.

Não valida lowering semântico completo, estrangeira/forced unwind com owners
do catch, finally/cancelamento/suspensão, SEH/statepoints, §§27–34 completos
ou desempenho ARC/A0. A prova é do protótipo de HIR preparado, com runtime real.

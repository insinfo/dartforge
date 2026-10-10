# ARC resume — AOT Linux da CI

Fonte `f626bb6887cdefb079eeba0910625111905e63bc`,
[CI 38010046028](https://github.com/insinfo/dartforge/actions/runs/38010046028),
job Linux `114087818147`. O passo da prova terminou com sucesso; o workflow
inteiro ainda estava ativo ao congelar os dados.

Quatro execuções reais do runtime: ARC/tracing × normal/erro, geradas pelo
exemplo `arc_retornos_guardados` com argumento `automatico`, LLVM embutido
O2. Executáveis sob `ARC_CONFERIR=1 BER=0 GC_STRESS=1`. Normal imprime
`9223372036854775807` seguido de LF; erro deixa stdout vazio. Todos saíram
com código zero. Stdout foi comparado byte a byte na CI e após o download.

A variante usa Mint real produzido pela factory Owned auditada. O propagador
possui uma cópia local, chama a função com retorno Guarda e recebe invoke,
pouso e drops pelo produtor ARC. IR arquivado confirma personalidade Itanium
de cleanup, landingpad cleanup e release antes de resume do par recebido,
sem df.lancar nesse pouso. A função chamada ainda publica a pendência e
inicia o unwind; o caller tem catch explícito que limpa a pendência.

`prova.json` registra hashes dos oito arquivos, das fontes no commit e o
digest do artefato Actions. IR é entrada do gerador, não IR otimizado.
Executáveis, logs completos e a versão exata da libLLVM não estão no artefato.

Reprodução em Linux com a toolchain do repositório:

```sh
cargo run --locked --release -p dartforge-emit-native --features llvm-embutido \
  --example arc_retornos_guardados -- target/prova-resume arc erro automatico
ARC_CONFERIR=1 BER=0 GC_STRESS=1 target/prova-resume
```

Trocar `arc` por `tracing` e `erro` por `normal` produz os demais casos.

## Limites

HIR manual; não certifica lowering semântico completo. Não observa diretamente
identidade do objeto de unwind, rastro ou morte final; não exercita estrangeira,
forced unwind, catch no perfil novo, finally/cancelamento/suspensão, SEH ou
statepoints. A variante `misto` foi acrescentada depois desta fonte e não está
nestes arquivos. Não mede desempenho ARC/A0 e não conclui a especificação ARC.

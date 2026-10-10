# Cleanup Itanium e resume: prova Linux

`runtime-prova.rs` contém os trechos reais do leitor LSDA e das personalidades
Itanium de `crates/runtime/src/excecoes_tabelas.rs`, mais uma fixture sem
dependências externas. `evidencias.json` registra hashes da fonte completa,
dos trechos congelados, do IR e dos executáveis; estes ficam em target.

O IR cria uma exceção nativa com a classe Dart, dois pousos `cleanup` aninhados
e um catch-all externo. Cada pouso verifica o mesmo objeto e seletor zero;
a fixture exige a sequência 1→2 e exatamente uma execução por pouso. Ambos
fazem `resume` com o par recebido. O handler exige o objeto original, seletor
1 e dois cleanups completos. Retorno zero prova esses invariantes; 10–13
identificam falha nos marcadores, objeto, seletor ou contagem; 99 indica que
a chamada falível retornou normalmente.

As compilações LLVM O0 e O2 executaram em Linux x86-64 na distribuição WSL
WSLg-Builder, código 0 em ambas. Rust está otimizado nas duas variantes.
As funções de lançamento/cleanup são noinline; isto não testa inlining.
O `resume` retorna diretamente ao desenrolador, sem `df.lancar` no cleanup.
A personalidade nova não reinicializa o objeto entre os pousos.

Reprodução em Linux com Rust e Clang:

```sh
rustc --edition 2024 --crate-type staticlib --crate-name personalidade_itanium -O runtime-prova.rs -o libruntime-prova.a
clang -O0 prova.ll libruntime-prova.a -ldl -lpthread -lm -o prova-O0
./prova-O0
clang -O2 prova.ll libruntime-prova.a -ldl -lpthread -lm -o prova-O2
./prova-O2
```

Na execução registrada, Rust e Clang 22.1.8 geraram os objetos Linux a partir
do Windows; GCC existente no WSL ligou os executáveis. Não houve instalação
de pacotes. O aviso de sessão systemd do WSL não impediu a ligação/execução.

O emissor ARC continua no protocolo legado. Esta prova não usa heap,
pendência ou rastro Dart, não insere drops ARC e não certifica a integração
com o frontend. A nova personalidade aceita cleanup puro e catch-all
separados; ações tipadas, pousos mistos e statepoints ainda não são cobertos.
Exceções estrangeiras e forced unwind foram testados somente na decisão de
fase em unidade. macOS tem prova de compilação de metadados, sem execução;
SEH/funclets continuam pendentes. O objetivo ARC completo permanece aberto.

Referência: [Exception Handling in LLVM — Cleanups](https://llvm.org/docs/ExceptionHandling.html#cleanups).

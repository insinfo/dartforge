# Itanium: cadeias de ações, pousos mistos e inlining

A personalidade real percorre os registros SLEB128 da cadeia de ações,
separando índice de ação, seletor de catch e presença de cleanup. Os elos
são relativos ao início do campo de deslocamento e podem apontar para trás.
Um registro de filtro zero pode representar cleanup mesmo com índice de
sítio positivo. O seletor entregue ao pouso é o filtro, não esse índice.

`runtime-prova.rs` congela os trechos reais do runtime e uma fixture sem
heap Dart. Os quatro casos foram executados em Linux x86-64 via WSL,
LLVM O2 e Rust otimizado, todos com código 0:

| Caso | Fluxo verificado |
|---|---|
| puro | Dois cleanups separados, seletor zero, resume e handler Dart externo. |
| inline | Ambos os cleanups são incorporados em main; pousos mistos recebem seletor 1 e executam uma vez cada antes do handler Dart. |
| estrangeira | Objeto de classe estrangeira passa pelos pousos mistos Dart com seletor zero, retoma e chega ao handler C++ `__gxx_personality_v0`. |
| estrangeira-inline | Os dois cleanups são incorporados na camada Dart; a verificação do seletor impede captura como Dart e retoma até o handler C++ externo. |

Todos verificam o mesmo ponteiro de objeto, os seletores e a sequência
cleanup interno id=1 → externo id=2, exatamente uma execução de cada.
A exceção estrangeira é um objeto nativo da fixture com classe estrangeira;
o handler externo usa a personalidade C++ real, não uma simulação dela.
Não é uma exceção produzida por `throw` C++ com objeto de linguagem.
Os arquivos `.opt.ll` comprovam a eliminação das funções de cleanup nas
variantes inline, os pousos mistos e a verificação do seletor antes de resume.

Reprodução em Linux com Rust, Clang e libstdc++:

```sh
rustc --edition 2024 --crate-type staticlib --crate-name personalidade_itanium -O runtime-prova.rs -o libruntime-prova.a
clang -O2 estrangeira-inline.ll libruntime-prova.a -ldl -lpthread -lm -lstdc++ -o prova
./prova
```

Trocar o arquivo IR reproduz os outros três casos. Na execução registrada,
Rust e Clang geraram os objetos Linux no Windows e GCC no WSL ligou os
executáveis, mantidos em target. Hashes estão em `evidencias.json`.

O emissor ARC ainda usa o protocolo legado. Não há heap, pendência, rastro,
drops ARC ou fonte Dart nesta fixture. Filtros tipados/negativos, statepoints,
SEH/funclets e integração no pipeline continuam pendentes. Forced unwind
foi conferido apenas na decisão unitária; macOS apenas como metadados.
Nenhuma nova medição de desempenho foi realizada.

Referências: [LLVM — Exception Handling](https://llvm.org/docs/ExceptionHandling.html#restrictions),
[libcxxabi — formato e encadeamento das ações](https://github.com/llvm/llvm-project/blob/main/libcxxabi/src/cxa_personality.cpp).

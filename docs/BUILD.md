# Ambiente de construção e velocidade do ciclo de desenvolvimento

Este documento cobre o que é preciso para construir o DartForge e o que foi
medido sobre o custo de construí-lo. Ele trata do **compilador Rust que constrói
a ferramenta**, não do desempenho do DartForge compilando Dart — esse está em
[DESEMPENHO.md](DESEMPENHO.md).

## Toolchain exigido

| Componente | Versão | Para quê |
| --- | --- | --- |
| Rust estável | fixada em `rust-toolchain.toml` | construção normal |
| Rust nightly | 1.100.0 ou posterior | backend Cranelift, recurso instável do Cargo |
| `rustc-codegen-cranelift-preview` | componente do nightly | acelerar o ciclo de edição |
| LLVM completo | 22.1.8, MSVC | `llvm-sys`, exigido por `crates/jit` |
| Clang | qualquer, no `PATH` ou em `DARTFORGE_CLANG` | driver AOT |
| Node.js | qualquer recente | testes diferenciais do backend JavaScript |
| Dart SDK | 3.6.2 e 3.13.4 | oráculos dos testes de conformidade |

### LLVM

`llvm-sys` exige `llvm-config`, os cabeçalhos `llvm-c/**` e as bibliotecas
estáticas. O instalador `LLVM-<versão>-win64.exe` **não serve**: ele traz apenas
clang e `LLVM-C.dll`. É preciso a distribuição completa
`clang+llvm-<versão>-x86_64-pc-windows-msvc`.

`.cargo/config.toml` define `LLVM_SYS_221_PREFIX` para todo o repositório. Uma
variável exportada no ambiente tem precedência, para quem instalar em outro
lugar.

### Cranelift como backend do rustc

```
rustup toolchain install nightly --profile minimal -c rustfmt -c clippy \
       -c rustc-codegen-cranelift-preview
```

Não confundir com o **Cranelift usado como biblioteca** dentro do DartForge,
em `crates/cranelift-jit`, que gera código Dart. São camadas diferentes: uma
constrói a ferramenta, a outra é a ferramenta. Usar Cranelift como biblioteca
não exige construir o DartForge com cg_clif.

## O que foi medido

Construção completa de `dartforge-compiler` e suas dependências, em diretório de
destino separado, no nightly com LLVM: **3m06s**. A máquina tinha quatro agentes
compilando em paralelo — 12 processos `cargo` e 8 `rustc` simultâneos —, então
esse número mede a máquina saturada, não o backend.

A comparação com Cranelift ficou **pendente**: a tentativa parou em 20 segundos
por erros de compilação do próprio código-fonte, vindos de trabalho concorrente
em andamento, e não por limitação do backend. Um número obtido assim não
descreveria coisa alguma.

## Duas observações que mudam a recomendação usual

### O gargalo atual é contenção de trava, não geração de código

Com vários processos compartilhando o mesmo `target/`, boa parte do tempo
aparece como `Blocking waiting for file lock on build directory`. Trocar o
backend não resolve isso; dar um `CARGO_TARGET_DIR` próprio a cada processo
concorrente resolve, ao custo de não compartilhar artefatos.

### O arranjo conservador não ajuda um ciclo dominado por testes

A recomendação usual é Cranelift em `[profile.dev]` e LLVM explicitamente em
`[profile.test]`, porque `test` **herda** de `dev` e configurar só `dev` não
mantém os testes no LLVM. Isso é correto para um ciclo de `cargo build` e
`cargo run`.

Só que `cargo test` usa `[profile.test]`. Num ciclo dominado por
`cargo test --workspace`, como o deste projeto, esse arranjo não acelera nada e
ainda passa a manter **dois conjuntos de artefatos**, um por backend. Antes de
adotá-lo, vale medir qual comando domina o seu ciclo.

### Por que os testes não ficam no Cranelift

No Windows x86-64 o cg_clif compila normalmente, mas **não suporta unwinding de
panics**. O executor padrão de testes do Rust depende de unwinding para isolar
a falha de um teste sem derrubar o processo. Existe `-Zpanic-abort-tests`, que
roda cada teste em um subprocesso, mas isso troca o modelo de execução em vez de
implementar unwinding: `catch_unwind` continua sem capturar nada.

`panic!` e unwinding não são a mesma coisa. Com `panic = "abort"` o processo
termina; com `panic = "unwind"` a pilha é desenrolada, os `Drop` executam e o
panic pode ser capturado. Tratamento de erro por `Result` e `?` não depende de
nada disso.

## Configuração para quando o ciclo for de edição, não de testes

Ainda **não** está ativa, porque exige fixar o nightly para todo o repositório e
há trabalho concorrente em curso. Para ativar, acrescente ao
`.cargo/config.toml` e mude `rust-toolchain.toml` para o canal nightly:

```toml
[unstable]
codegen-backend = true

[profile.dev]
codegen-backend = "cranelift"
panic = "abort"

# `test` herda de `dev`: sem esta seção, os testes iriam para o Cranelift e
# perderiam o unwinding de que o executor depende.
[profile.test]
codegen-backend = "llvm"

[profile.release]
codegen-backend = "llvm"
```

`panic = "abort"` em `dev` é consequência do backend, não preferência: no
Windows o cg_clif não oferece a alternativa. Antes de adotar, confirme que
nenhuma parte do DartForge depende de recuperar panics para manter uma sessão
aberta — hoje nada usa `catch_unwind`, o que torna a troca viável.

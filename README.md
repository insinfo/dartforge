# DartForge

[![CI](https://github.com/insinfo/dartforge/actions/workflows/ci.yml/badge.svg)](https://github.com/insinfo/dartforge/actions/workflows/ci.yml)

Compilador experimental **Dart 3.6.2 → JavaScript**, escrito em Rust, sob licença MIT.
Objetivo: uma base compartilhada para compilação, análise, LSP, ngdart e ferramentas web.

## Estado atual

O protótipo suporta um subconjunto explícito:

- Uma entrada `void main()`, funções tipadas e parâmetros posicionais obrigatórios.
- Chamadas, recursão, `return`, `if/else` e laços `while`, `do/while`, `for` com blocos obrigatórios.
- `break`/`continue` sem rótulos; `++`, `--`, `+=`, `-=` e `*=` sobre identificadores, como instruções.
- Variáveis locais `var`, `final`, `int`, `String`, `bool`, atribuições e escopos.
- Expressões aritméticas, relacionais e booleanas; `print`; strings Unicode com escapes e strings raw.
- Validação de nomes, tipos, argumentos, retornos e mutabilidade.
- Emissão JavaScript ESM e testes diferenciais com o SDK Dart 3.6.2.

Ainda não compila aplicações Dart/ngdart completas. Não há classes, imports, bibliotecas
padrão completas, LSP ou servidor web implementados. Não há benchmarks que demonstrem
vantagem sobre DDC/dart2js. Veja [o subconjunto](docs/SUBCONJUNTO.md) e [o roteiro](PLANO.md).

## Compilar e testar

Instale Rust via rustup (a versão é fixada em rust-toolchain.toml). Node 24 é usado pelos
testes de execução; Dart 3.6.2 é usado pelas comparações diferenciais.

    cargo build --locked --release
    cargo run -p dartforge-cli -- compile examples/hello/main.dart dist/hello.mjs
    node dist/hello.mjs
    cargo test --locked --workspace

O CLI recusa sobrescrever arquivos existentes. Use outra saída ao repetir.

No PowerShell:

    ./scripts/check.ps1
    cargo test --locked --workspace -- --include-ignored
    ./scripts/conformance.ps1

O runner usa `-O2` por padrão e exige Dart 3.6.2. Para investigar diferenças entre níveis:

    ./scripts/conformance.ps1 -OptimizationLevels O0,O1,O2,O3

Uma divergência produz relatório e falha explícita. O nível `-O0` apresenta uma divergência
numérica conhecida no corpus; veja [a política de referência](docs/DART2JS-REFERENCIA.md).
Não tratamos um compilador específico como oráculo absoluto.

Na máquina de desenvolvimento original, Rust está em `D:\Rust` e o projeto em
`D:\Projects\dartforge`; `scripts/env.ps1` usa essa instalação quando presente.

## Workspace

| Crate | Responsabilidade |
|---|---|
| diagnostics | Diagnósticos e spans |
| syntax | Tokens e AST |
| lexer / parser | Frontend do subconjunto |
| semantic | Resolução local, funções e tipos |
| hir | Representação estrutural intermediária |
| codegen | Emissão JavaScript |
| compiler / cli | Pipeline compartilhado e executável |
| ngdart / lsp / web | Fronteiras iniciais para expansão |

## Contribuição e documentação

Siga [CONTRIBUTING.md](CONTRIBUTING.md). Comentários e documentação Rust são escritos
em português: `//!` descreve módulos e `///` documenta funções, contratos e limitações.
Exemplos de APIs públicas são verificados como doctests. Para gerar a documentação:

    cargo doc --locked --workspace --no-deps --document-private-items

## Referências e licença

Catálogo: [docs/REFERENCIAS.md](docs/REFERENCIAS.md).
Revisões dos clones locais: [docs/references-manifest.json](docs/references-manifest.json).
Avaliação do Rust Sitter: [docs/RUST-SITTER.md](docs/RUST-SITTER.md).

A pasta `references/` inteira é ignorada pelo Git. Nenhum clone de terceiro é publicado
neste repositório nem usado automaticamente como dependência do build.
O código original está sob [MIT](LICENSE); projetos de referência mantêm suas licenças.

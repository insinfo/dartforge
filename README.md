# DartForge

[![CI](https://github.com/insinfo/dartforge/actions/workflows/ci.yml/badge.svg)](https://github.com/insinfo/dartforge/actions/workflows/ci.yml)

Compilador experimental **Dart 3.6.2 → JavaScript e executáveis nativos LLVM**, escrito em Rust, sob licença MIT.
Objetivo: uma base compartilhada para compilação, análise, LSP, ngdart e ferramentas web.

## Estado atual

O protótipo suporta um subconjunto explícito:

- Uma entrada `void main()`, funções tipadas e parâmetros posicionais obrigatórios.
- Chamadas, recursão, `return`, `if/else` e laços `while`, `do/while`, `for` com blocos obrigatórios.
- `break`/`continue` sem rótulos; `++`, `--`, `+=`, `-=` e `*=` sobre identificadores, como instruções.
- Variáveis locais `var`, `final`, `int`, `String`, `bool`, atribuições e escopos.
- Expressões aritméticas, relacionais e booleanas; `print`; strings Unicode com escapes e strings raw.
- Tipos anuláveis, promoção de fluxo, operadores `??` e `!`.
- Classes, campos, métodos e herança simples com despacho dinâmico.
- Extensions nomeadas em uma unidade, com alvos de chamadas resolvidos estaticamente.
- Imports relativos e `package:`, filtros `show`/`hide` e reexports cíclicos com privacidade.
- Sessão com cache da última saída validado pelo conteúdo integral do grafo.
- Validação de nomes, tipos, argumentos, retornos e mutabilidade.
- Avaliação opcional de constantes puras com `--optimize`.
- Fusão conservadora de funções idênticas com `--merge-identical-functions` (opt-in).
- Emissão JavaScript ESM e testes diferenciais com o SDK Dart 3.6.2.

Ainda não compila aplicações Dart/ngdart completas. Faltam generics, construtores explícitos, bibliotecas
padrão completas, LSP e servidor web. Não há benchmarks que demonstrem
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

## Análise, imports e desempenho

Detalhes: [null safety](docs/NULL-SAFETY.md), [classes](docs/CLASSES.md),
[bibliotecas/imports](docs/MODULES.md), [extensions](docs/EXTENSIONS.md), [pacotes](docs/PACKAGES.md), [cache](docs/CACHE.md) e [benchmarks](docs/BENCHMARKS.md).

    cargo run -p dartforge-cli -- graph caminho/main.dart
    cargo bench --locked -p dartforge-compiler --bench pipeline

O comando `graph` mostra dependências; `compile` resolve e compila bibliotecas relativas e pacotes configurados.

## Backend AOT LLVM inicial

    ./scripts/env.ps1
    cargo run --release -p dartforge-cli -- emit-llvm examples/native/main.dart dist/native.ll
    cargo run --release -p dartforge-cli -- aot examples/native/main.dart dist/native.exe --optimize

O AOT suporta `int` de 64 bits com overflow modular, `bool`, funções/recursão,
variáveis, condicionais, laços, imports/pacotes e null safety (`int?`, `bool?`, `String?`, classes anuláveis, `??`, `!`).
Strings, classes com construtor implícito, herança e despacho virtual usam um runtime
Rust com GC preciso que coleta ciclos. Extensions ainda recebem diagnóstico nativo.
Raízes usam slots reutilizáveis por ativação, sem acumular entradas a cada iteração.
Valores antigos ainda podem ficar retidos até sobrescrita do slot ou retorno.
Veja o [contrato do runtime](crates/runtime/README.md) e as [referências Swift](docs/SWIFT-REFERENCIAS.md).

Funções e métodos aceitam corpos tipados `=>`, por exemplo `int soma(int a, int b) => a + b;`.
A opção `--merge-identical-functions` funciona em `compile`, `emit-llvm` e `aot`,
independentemente de `--optimize`. Ela compara estrutura e assinaturas, redireciona
chamadas e remove definições redundantes; não busca equivalência algébrica geral.
Consulte [o contrato e os benchmarks da fusão](docs/MERGE-FUNCOES.md).

Requer Clang/LLVM 17+ e rustc/linker nativo. Configure `DARTFORGE_CLANG` e
`DARTFORGE_RUSTC` ou use as ferramentas no PATH. Na instalação original,
`scripts/env.ps1` detecta LLVM 22.1.8 em `D:\LLVM\22.1.8`.
Sem `--optimize`, usa LLVM O0; com a opção, LLVM O2. Isso é independente do
passe de constantes do backend JS. O runtime é Rust; LLVM/Clang são dependências
externas. O executável não requer o Dart SDK para rodar.

`--timings` em `aot` emite JSON com tempos do frontend, preparação, Clang,
compilação/link do runtime Rust, publicação e total, além do tamanho do executável.
A falha de `!` sobre null encerra o processo com diagnóstico; ainda não há exceções
Dart capturáveis no alvo nativo. Veja [null safety AOT](docs/AOT-NULL-SAFETY.md).

Detalhes: [driver e ABI](docs/AOT-DRIVER.md), [referências Dartino](docs/AOT-REFERENCIAS.md)
e [incremento atual de raízes reutilizáveis e fusão](docs/IMPLEMENTACAO-10.md).

## Workspace

| Crate | Responsabilidade |
|---|---|
| packages / linker | Grafo, namespaces, privacidade e ligação de bibliotecas |
| diagnostics | Diagnósticos e spans |
| syntax | Tokens e AST |
| lexer / parser | Frontend do subconjunto |
| semantic | Resolução local, funções e tipos |
| hir | Representação estrutural intermediária |
| codegen | Emissão JavaScript |
| optimizer | Simplificação opcional de constantes após validação |
| compiler / cli | Pipeline compartilhado e executável |
| llvm / native / runtime | LLVM IR, driver AOT e runtime Rust inicial |
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

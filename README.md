# DartForge

[![CI](https://github.com/insinfo/dartforge/actions/workflows/ci.yml/badge.svg)](https://github.com/insinfo/dartforge/actions/workflows/ci.yml)

Compilador experimental **Dart → JavaScript e executáveis nativos LLVM**, escrito em Rust, sob licença MIT.
Meta governante: compilar qualquer projeto Dart 3.6 válido no dart2js/DDC, com a mesma semântica do Dart oficial.
Objetivo: uma base compartilhada para compilação, análise, LSP, ngdart e ferramentas web.

O que funciona, verificado por execução, e o que falta: **[ESTADO.md](ESTADO.md)**.
Decisões e roteiro: **[PLANO.md](PLANO.md)**.

## Estado em uma tela

- **JavaScript de desenvolvimento** (`compile-js`, `dev`, `serve`): módulos ES no contrato do
  DDC, ligados ao `dart_sdk.js` oficial. Corpus diferencial inteiro igual à VM, byte a byte;
  `new_sali/frontend` (ngdart) e `limitless_ui/example` rodam no navegador.
- **JavaScript de produção** (`dartforge-jsprod`): um arquivo, runtime podado, mundo fechado.
- **Nativo AOT** (`compile-native`/`aot`, feature `nativo`) e **JIT** ORCv2 (`run`/`reload`,
  feature `jit`) sobre o mesmo LLVM IR, com runtime Rust e GC por tracing. Em andamento.
- **LSP** (`crates/lsp` + `editors/vscode`) e o **gerador do ngdart** (`.template.dart` sem
  `build_runner`).

## Compilar e usar

Rust é fixado em `rust-toolchain.toml`. O SDK Dart 3.6.2 é o oráculo e fornece o `lib/` do SDK;
Node 24 executa a saída JavaScript.

    pwsh scripts/gerar-dart-sdk.ps1           # dart_sdk.js (uma vez)
    cargo build --release -p dartforge-cli
    cargo run --release -p dartforge-cli -- compile-js examples/hello/main.dart -o dist/hello
    cargo run --release -p dartforge-cli -- dev <entrada.dart> -o <saida> --packages <cfg>

Backend nativo (Clang/LLVM 22.1.8; `scripts/env.ps1` configura o ambiente):

    cargo run --release -p dartforge-cli --features nativo -- compile-native <entrada.dart> -o <saida.exe>
    cargo run --release -p dartforge-cli --features jit -- run <entrada.dart>

O `jit` exige a distribuição completa do LLVM (`LLVM_SYS_221_PREFIX`, em `.cargo/config.toml`)
e a `LLVM-C.dll` no `PATH`; ver [docs/JIT.md](docs/JIT.md).

## Verificar

    cargo test --locked --workspace
    cargo run --release -p dartforge-diferencial               # corpus JS contra a VM e o dartdevc
    cargo run --release -p dartforge-diferencial -- --producao
    pwsh scripts/ci.ps1 -Frente <nome> -Acompanhar            # CI completo (ci.yml + pesado.yml)

O que roda no CI e o que fica de fora estão no `ESTADO.md` §1.6 e §3.

## Workspace

| Crate | Responsabilidade |
|---|---|
| frontend | Léxico, sintaxe e parser de Dart 3.6 (sem resolução) |
| elements / types | Modelo de elementos, SDK a partir da fonte, tipos, inferência e fluxo |
| emit_js | Emissão JavaScript no contrato do DDC |
| mundo / emit_js_producao | Mundo fechado e perfil de produção (um arquivo) |
| gerador_ng | Gerador dos `.template.dart` do ngdart |
| emit_native / runtime / jit | HIR própria → LLVM IR → Clang; runtime Rust; JIT ORCv2 |
| abi | Perfis de ABI escalar (`abi-info`) |
| dev | Compilador residente (`dev`, `serve`) |
| lsp | Servidor LSP |
| diferencial | Harness diferencial contra a VM, o `dartdevc` e o próprio AOT |
| intern / diagnostics / instrument | Símbolos, diagnósticos e medição |
| cli | O executável `dartforge` |

A exploração inicial (trilha velha `lexer`/`parser`/`semantic`/`hir`/`codegen`/`llvm`,
`compiler`, `cranelift-jit` e afins) foi removida e está preservada na branch
`exploracao-inicial`; os documentos dela estão em [docs/historico](docs/historico/README.md).

## Contribuição e documentação

Siga [CONTRIBUTING.md](CONTRIBUTING.md). Comentários e documentação Rust são escritos
em português: `//!` descreve módulos e `///` documenta funções, contratos e limitações.

    cargo doc --locked --workspace --no-deps --document-private-items

## Referências e licença

Catálogo: [docs/REFERENCIAS.md](docs/REFERENCIAS.md).
Revisões dos clones locais: [docs/references-manifest.json](docs/references-manifest.json).

A pasta `references/` inteira é ignorada pelo Git. Nenhum clone de terceiro é publicado
neste repositório nem usado automaticamente como dependência do build.
O código original está sob [MIT](LICENSE); projetos de referência mantêm suas licenças.
Avisos de componentes: [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

# Documentos históricos

Estes documentos descrevem a **exploração inicial** do DartForge: a trilha
velha (`lexer` → `parser` → `semantic` → `hir` → `codegen`/`llvm`, com
`linker`, `optimizer`, `packages`, `macros`, `compiler` e o driver
`native`) e os experimentos de backend (`cranelift-jit`, `asmjit-jit`).
Esses crates saíram do workspace; o código, os testes e os scripts que eles
citam estão preservados na branch **`exploracao-inicial`**
(`git switch exploracao-inicial`).

Nada aqui é contrato vigente. O que vale hoje está em `ESTADO.md`,
`PLANO.md` e nos documentos de `docs/` fora desta pasta. Os caminhos de
código citados (`crates/compiler/...`, `scripts/conformance.ps1`,
`tests/conformance/...`) só existem naquela branch.

* `IMPLEMENTACAO-01.md` a `IMPLEMENTACAO-25.md` — os incrementos da
  trilha velha, na ordem do `PLANO.md`.
* Contratos por recurso da trilha velha (`SUBCONJUNTO.md`, `NUCLEO.md`,
  `CLASSES.md`, `TIPOS.md`, `STRINGS.md` etc.) e as referências levantadas
  para cada incremento (`*-REFERENCIAS.md`, `RUST-SITTER.md`).
* `CRANELIFT.md` e `ASMJIT.md` — os experimentos de backend; a conclusão
  (não adotar Cranelift) continua citada no `ESTADO.md` e no `PLANO.md`.
* `BUILD.md` — medições de construção do workspace antigo.
* `dados/` — relatórios JSON de conformidade e benchmarks daqueles
  incrementos.
* `briefs/` — ordens de serviço de agentes já cumpridas (também as da
  trilha nova: elements, types, inferência, LSP, nativo, JS de produção).

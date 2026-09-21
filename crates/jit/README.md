# JIT ORCv2 — perfil de desenvolvimento

Executa em memória, via LLVM ORCv2 (`LLJIT`), o mesmo LLVM IR que `crates/native`
compila para executável. Não há emissor próprio: a entrada é o IR textual de
`dartforge_compiler::compile_llvm_with_options`. A arquitetura, os limites e a
justificativa de ORCv2 antes de Cranelift estão em [docs/JIT.md](../../docs/JIT.md).

```rust
let ir = dartforge_compiler::compile_llvm("void main() { print(7); }")?;
let (saida, relatorio) = dartforge_jit::run_ir_capturing(&ir)?;
assert_eq!(saida, "7\n");
```

Pela CLI: `dartforge run <entrada.dart> [--timings]`.

## Requisito de build e de execução

O crate usa `llvm-sys` 221.1.0 para as assinaturas da API C. Ele localiza o LLVM
22.1.x pelo `llvm-config` da **distribuição completa** (a que traz
`bin/llvm-config.exe`, `include/llvm-c/**` e as bibliotecas). O instalador
reduzido de Windows, com apenas `LLVM-C.dll` e `LLVM-C.lib`, não serve.

`.cargo/config.toml` na raiz já define `LLVM_SYS_221_PREFIX`, então qualquer
`cargo` dentro do repositório enxerga a variável. Uma definição no ambiente do
shell tem precedência. `DARTFORGE_LLVM_DIR` também é aceita pelo `build.rs`
deste crate, mas o `build.rs` do `llvm-sys` só conhece `LLVM_SYS_221_PREFIX`;
mantenha as duas iguais. Sem um prefixo válido a build do workspace inteiro
falha, porque este crate é membro de `crates/*`.

A ligação é **dinâmica**, contra `LLVM-C.dll` (`no-llvm-linking` +
`build.rs` próprio). As bibliotecas estáticas do pacote oficial de Windows usam
CRT estática e conflitam com a CRT dinâmica do Rust — o efeito medido é um
`STATUS_ACCESS_VIOLATION` ao liberar uma mensagem de erro do LLVM. O `build.rs` e
[docs/JIT.md](../../docs/JIT.md) trazem a evidência.

Consequência: **`LLVM-C.dll` precisa estar alcançável pelo carregador** para
executar. `scripts/env.ps1` acrescenta `<prefixo>/bin` ao `PATH`. Os testes que
abrem uma `LLJIT` são `#[ignore]` com esse motivo; rode-os com
`cargo test -p dartforge-jit -- --include-ignored`.

## Fronteira `unsafe`

Todo o `unsafe` do crate está em `src/ffi.rs`, único arquivo com
`#![allow(unsafe_code)]`; `src/lib.rs` e `src/runtime.rs` permanecem sob
`unsafe_code = "deny"` do workspace, de modo que qualquer `unsafe` novo fora da
fronteira é erro de compilação. Cada bloco documenta a invariante que preserva.

## Limites em uma linha

O programa executa **dentro do processo do compilador**: sem isolamento, com o
heap gerenciado da thread e o stdout do hospedeiro. Um programa por sessão, porque
todos definem `@dartforge_entry`. Sem hot reload, sem `@Native` de `dart:ffi`.

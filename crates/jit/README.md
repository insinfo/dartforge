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

## Requisito de build: `LLVM_SYS_221_PREFIX`

O crate liga `llvm-sys` 221.1.0, que localiza o LLVM 22.1.x pelo `llvm-config` da
**distribuição completa** (a que traz `bin/llvm-config.exe`, `include/llvm-c/**`
e as bibliotecas). O instalador reduzido de Windows, com apenas `LLVM-C.dll` e
`LLVM-C.lib`, não serve.

```pwsh
$env:LLVM_SYS_221_PREFIX = 'D:/DartSDKs/llvm/clang+llvm-22.1.8-x86_64-pc-windows-msvc'
cargo build --workspace
```

Sem a variável (e sem `llvm-config` compatível no `PATH`) a build do workspace
inteiro falha, porque este crate é membro de `crates/*`. `DARTFORGE_LLVM_DIR`
continua sendo o nome preferido do projeto para apontar um prefixo LLVM, mas quem
procura o `llvm-config` é o `build.rs` do `llvm-sys`, que só conhece
`LLVM_SYS_221_PREFIX`; defina as duas com o mesmo valor.

Com a ligação estática padrão não é preciso ter `LLVM-C.dll` no `PATH` para rodar
os testes. Em modo dinâmico (`--features llvm-sys/force-dynamic`, ou distribuição
sem bibliotecas estáticas), é.

## Fronteira `unsafe`

Todo o `unsafe` do crate está em `src/ffi.rs`, único arquivo com
`#[allow(unsafe_code)]`; `src/lib.rs` e `src/runtime.rs` permanecem sob
`unsafe_code = "deny"` do workspace, de modo que qualquer `unsafe` novo fora da
fronteira é erro de compilação. Cada bloco documenta a invariante que preserva.

## Limites em uma linha

O programa executa **dentro do processo do compilador**: sem isolamento, com o
heap gerenciado da thread e o stdout do hospedeiro. Um programa por sessão, porque
todos definem `@dartforge_entry`. Sem hot reload, sem `@Native` de `dart:ffi`.

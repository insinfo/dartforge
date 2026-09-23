# JIT ORCv2 — perfil de desenvolvimento

Executa em memória, via LLVM ORCv2 (`LLJIT`), o **mesmo LLVM IR textual** que o
AOT de `crates/emit_native` entrega ao Clang. Não há emissor próprio. A
arquitetura, os limites e a justificativa de ORCv2 antes de Cranelift estão em
[docs/JIT.md](../../docs/JIT.md).

```rust
let relatorio = dartforge_jit::run_ir(&ir)?;   // imprime no stdout do processo
assert_eq!(relatorio.entry.exit_code, 0);
```

Executor isolado, um processo por programa (é o que testes e harness usam):

```
dartforge-executar-ir programa.ll [--timings]
```

stdout e código de saída são os do programa, como no executável AOT;
`--timings` escreve um JSON em stderr. Falha do próprio JIT sai com código 70.

## Contrato

* **Entrada:** LLVM IR textual do `emit_native`, com `@dartforge_entry`.
* **Runtime:** a fonte do harness AOT, `crates/runtime/src/runtime_main.rs`.
  **Provisório:** até a fonte única entrar em `crates/runtime`, o `build.rs`
  deste crate gera uma cópia mecânica dela, com três substituições conferidas,
  e a tabela `(nome, endereço)` a partir dos `#[unsafe(no_mangle)]`. Nenhuma
  função do runtime é reescrita aqui.
* **Símbolos:** o runtime entra como símbolos absolutos. Todo nome que o IR
  declara precisa estar na tabela do runtime, na lista de CRT
  (`CRT_SYMBOLS`), ser `llvm.*` ou ser definido por um módulo da sessão. Se
  não estiver, a sessão recusa o módulo na etapa `símbolos`.
* **Alvo:** CPU `x86-64`, `CodeGenLevelNone`, que é o par do `clang -O0`. Um
  `target datalayout`/`triple` divergente do da `LLJIT` é recusado (`layout`).
* **Execução:** a entrada passa pelo mesmo `main` do harness AOT, numa thread
  nova com 1 MiB de pilha. O runtime começa zerado a cada execução.
  `process::exit` do runtime encerra o processo com o código do AOT.

## Requisito de build e de execução

O crate usa `llvm-sys` 221.1.0 contra a **distribuição completa** do LLVM
22.1.x (`LLVM_SYS_221_PREFIX`, já em `.cargo/config.toml`). A ligação é
**dinâmica**, contra `LLVM-C.dll`: as libs estáticas oficiais usam a CRT
estática. `scripts/env.ps1` põe `<prefixo>/bin` no `PATH`. Ponha a distribuição
completa **antes** de `D:/LLVM/22.1.8/bin`, que traz outra `LLVM-C.dll`.

Os testes que abrem uma `LLJIT` são `#[ignore]`. Rode-os com:

```
cargo test -p dartforge-jit -- --include-ignored
```

`jit_e_aot_concordam_no_mesmo_ir` exige também Clang, `rustc` e o SDK
Dart 3.6.2.

## Pendente

`testes-pendentes/hot_reload.rs` são os testes do hot reload escritos contra a
trilha velha, que não compilam mais. Eles usam `dartforge_compiler` e a captura
de saída em processo. Voltam quando forem migrados para o IR do `emit_native`
(plano do JIT, passo 11). O mecanismo em `src/reload.rs` continua coberto pelos
testes unitários do módulo.

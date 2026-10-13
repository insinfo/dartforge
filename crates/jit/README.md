# ORCv2 JIT — development profile

Runs in memory, via LLVM ORCv2 (`LLJIT`), the **same textual LLVM IR** that the
AOT of `crates/emit_native` hands to Clang. There is no emitter of its own. The
architecture, the limits and the rationale for ORCv2 over Cranelift are in
[docs/JIT.md](../../docs/JIT.md).

```rust
let report = dartforge_jit::run_ir(&ir)?;   // prints to the process's stdout
assert_eq!(report.entry.exit_code, 0);
```

Isolated executor, one process per program (it is what tests and the harness use):

```
dartforge-executar-ir program.ll [--timings]
```

stdout and exit code are the program's, as in the AOT executable;
`--timings` writes a JSON to stderr. A failure of the JIT itself exits with code 70.

## Contract

* **Input:** textual LLVM IR from `emit_native`, with `@dartforge_entry`.
* **Runtime:** the source of the AOT harness, the fragments of `crates/runtime/src` (listed in `crates/runtime/build.rs`),
  compiled as `dartforge_runtime::abi` (single source). The symbol table
  is `dartforge_runtime::symbols`, generated from the same files. No runtime
  function is rewritten here.
* **Symbols:** the runtime goes in as absolute symbols. Every name the IR
  declares must be in the runtime table, in the CRT list
  (`CRT_SYMBOLS`), be `llvm.*` or be defined by a module of the session. If
  it is not, the session refuses the module at the `symbols` stage.
* **Target:** CPU `x86-64`, `CodeGenLevelNone`, the counterpart of `clang -O0`. A
  `target datalayout`/`triple` diverging from the `LLJIT`'s is refused (`layout`).
* **Execution:** the entry is called on a new
  thread with 1 MiB of stack. The runtime starts zeroed on each execution.
  The runtime's `process::exit` ends the process with the AOT code.

## As a library: persistent session

```rust
let module = dartforge_jit::compile_module("macro", &ir)?;   // once; bytes in module.object()
let mut session = dartforge_jit::JitSession::new()?;          // once per process
session.add_compiled_module(&module)?;                         // no parsing nor generation
for _ in 0..n {
    let r = session.run_entry()?;                              // clean Dart state each time
    assert_eq!(r.exit_code, 0);
}
```

Costs measured in `docs/JIT.md` («Sessão persistente e cache de módulos»).

## Build and run requirement

The crate uses `llvm-sys` 221.1.0 against the **full distribution** of LLVM
22.1.x (`LLVM_SYS_221_PREFIX`, already in `.cargo/config.toml`). Linking is
**dynamic**, against `LLVM-C.dll`: the official static libs use the static
CRT. `scripts/env.ps1` puts `<prefix>/bin` on `PATH`. Put the full
distribution **before** `D:/LLVM/22.1.8/bin`, which brings another `LLVM-C.dll`.

The tests that open an `LLJIT` are `#[ignore]`. Run them with:

```
cargo test -p dartforge-jit -- --include-ignored
```

`jit_and_aot_agree_on_same_ir` also requires Clang, `rustc` and the Dart
3.6.2 SDK.

## Pending

`pending_tests/hot_reload.rs` holds the hot reload tests written against the
old track, which no longer compile: they use `dartforge_compiler`,
`dartforge_native` and the crates of the old front end (removed from the workspace and
preserved in the `exploracao-inicial` branch) and in-process output capture.
They stay here, outside `tests/`, as the list of scenarios to migrate. They come back when migrated to the `emit_native` IR
(JIT plan, step 11). The mechanism in `src/reload.rs` stays covered by the
module's unit tests.

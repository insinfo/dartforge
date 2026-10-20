# Current JIT verification and complete workspace collection — 2026-10-20

Base `dac116ab`. The older string-length report still described pruned AOT IR
execution in the JIT as an unresolved crash. The later SDK-owned runtime-data fix
(`a5e0962f`) already solved and verified that exact case. Its earlier failure
records are preserved; the historical report now identifies the follow-up.

## Reproduction and JIT suite

The unchanged historical development IR and its original matching SDK library
run with the newly built current executor, normally and with actual
DARTFORGE_GC_STRESS=1, printing the Dart 3.6.2 reference output `35294`. A separately
preserved executor also passes. That preserved executor's hash is different from
the executor in the original crash record: it is not an old failing negative
control. The original failing binary was previously replaced; no new claim of
before/after crash reproduction is made. The old IR and SDK hashes are retained.

The complete `cargo test --locked -p dartforge-jit -- --include-ignored` execution
passes **92 regular tests plus six doctests**, with zero ignored tests remaining.
The total comprises 45 unit, one async-frame reload, five callback-retention,
18 execution, 17 hot-reload, five persistent-session and one plain-SDK-residency
test. The existing measurement test is included for functional verification;
its timings are not a performance claim. The tests use two Rust test threads.

Runner publication uses a fresh inode and atomic rename before execution.
Compiler/executor/library/input bindings, the VM reference, complete test log
and selected environment are retained. The LLVM compression-object deployment
warning and missing /usr/local/lib linker-search warning remain visible. This
checks macOS ARM64, not an older-macOS or new Windows/Linux execution matrix.

## Workspace collection behavior

The local PowerShell checker and CONTRIBUTING command now use `--no-fail-fast`,
matching the ordinary CI command. Cargo continues to later test executables
after a failing executable, while retaining a nonzero final exit status. The
checker still throws on a nonzero test status. No test, ignore attribute,
assertion, filter, exclusion or approval criterion is weakened. PowerShell's
parser accepts the updated script; Cargo's installed help confirms the flag.

A full workspace execution has been launched with the new command, including
ignored tests and without excluding the known foreign-backend failure. Its
initial launch snapshot is retained here. That snapshot is not a completion
record. The live command/log are under `/tmp/forge-workspace-complete`, with
Cargo PID 19640 at launch and tool session 75651. Rust compilation and test
parallelism are each capped at two; stable external native/macro compiler paths
and Dart/Node/LLVM prerequisites are recorded. Temporary files stay on the SSD.

Workspace approval remains pending until that command terminates and all results
are inspected. The known foreign-backend absence gate and global formatting are
still open. This increment changes only verification instructions/reporting;
no Rust compiler or runtime source, SDK ABI or target bundle changed.

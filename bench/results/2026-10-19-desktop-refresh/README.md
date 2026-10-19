# Desktop validation refresh

Base compiler sources: `2b923a60`; real dart_ui File Explorer: `15aff704`;
Dart source SDK 3.6.2; LLVM 22.1.8. Windows/Linux compilers were rebuilt from an
archive of the base commit. macOS additionally includes the framework catalog
fix whose source hash is in `source-and-bundles.json`.

Both VMs exported fresh target bundles with matching runtime source fingerprint
`31967049c68737701387395423e70356b0547ad355d8013826187d019ec18fd1`.
The compiler correctly rejected older bundles instead of mixing runtime revisions.

The optimized macOS compiler initially failed to link the source SDK DLL because
its generated CoreFoundation stub omitted eight public exports. The release-mode
runtime link audit exposed one more CoreFoundation export and twenty Security
exports from archive objects pulled under this codegen configuration. All 29
exports were checked against the local Apple SDK 26.5; the five macOS link tests
now pass in release, including actual runtime executable/dynamic-library links.
The regular strict native Clippy still reports 33 preexisting library errors,
with none in `linker_macos.rs`; no suppression was added.

`macos.json` records all six successful cases after the fix: explorer AOT build,
AOT execution and JIT execution, plus context-menu smoke AOT build/execution and
JIT execution. Both smoke executions require `CONTEXT_RETARGET=PASS` and check
application errors. The initial JIT failure is retained separately.

`cross-windows-build.json` and `cross-windows-run.json` prove a macOS-built PE x64
was copied to Windows and executed successfully, with matching executable hash.
`cross-linux-build.json` and `cross-linux-run.json` prove the macOS-built ELF
ARM64 also executed successfully in Linux, with a matching executable hash.
The Windows menu smoke was additionally cross-built and executed with its pass marker.

The initial Linux run with the prior compiler passed explorer AOT/JIT and context
AOT, then context JIT was killed by the kernel while compiler rebuilding was also
active. `linux-oom.log` confirms the OOM kill. The Parallels control channel stopped
answering and a normal restart failed. The owner explicitly authorized a forced
reset. After recovery, the refreshed Linux compiler passed all six cases in
`linux.json`, with one compiler active at a time. The retained prior-compiler log
is not certification of the refreshed compiler.

Windows native AOT build/execution passed (`windows-aot.json`). Its first terminal
session was canceled before JIT started; those results were preserved. Separate
child processes launched from a script file passed explorer and context-menu JIT
(`windows-jit.json`). Both records use the same compiler hash. The current Windows
menu AOT execution is from the cross-built executable; its pass marker and hash
are recorded separately. No recurring task or resident validation agent was installed.

| Execution OS | Explorer AOT | Explorer JIT | Menu AOT / JIT |
|---|---|---|---|
| macOS ARM64 | pass | pass | pass / pass |
| Linux ARM64, Wayland/OpenGL | pass | pass | pass / pass |
| Windows 11 ARM64, x64 compatibility, D3D11 | pass | pass | pass (cross-built) / pass |

The refreshed compilers use low Rust release optimization for validation builds;
the Dart AOT programs still use `--optimize`. Both VMs have two CPUs and 4 GB RAM.
The four required root checks were repeated after the catalog change: format and
strict Clippy retain existing failures; default workspace testing cannot link
LLVM 22 bitcode with the Apple linker; documentation builds with warnings. See
`workspace-checks.json`. The earlier 815 native unit and 121 doctest successes are
recorded in the SDK inventory consumption evidence; the catalog change additionally
passed the five release link tests and the actual macOS desktop matrix.

Application entry-point checks run two frames and require normal process exit;
these are startup/rendering and menu regression smokes, not exhaustive visual,
long-duration or performance certification. Executables/bundles remain in
ignored `target/desktop-current/` and `target/cross-bundles/current/` directories.

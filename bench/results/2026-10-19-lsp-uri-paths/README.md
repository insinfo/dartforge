# LSP file URI completion — 2026-10-19

File completion used to treat percent-encoded directory segments as literal
filesystem names. Relative imports with `%20`, `%25` or `%23` and absolute file
URIs into paths containing spaces consequently missed existing Dart files.
The lookup now resolves a file URI against the source directory and decodes it
once, while preserving the editor's original prefix and raw suggested filename.
Existing package completion and the relative `lib` boundary check are retained.

## Validation

- `regressions-before.log`: the previous implementation fails the relative and
  absolute encoded-path regressions; the unescaped/package control passes.
- `lsp-tests.log`: all 258 LSP tests pass, including three integration tests and
  three lookup unit tests. The final run includes invalid UTF-8 rejection.
- `dart-oracle.json`: six completion responses from the official Dart 3.6.2
  language server, using the real SDK and a temporary project whose path has
  spaces. Labels and edit text preserve the typed URI prefix; newly suggested
  filenames retain spaces and Unicode. Only the temporary absolute root is
  normalized to `file:<fixture>` in the stored response.
- `dart-path-controls.stdout`: direct `Uri.toFilePath` observations from Dart
  3.6.2. Encoded separators, URI query/fragment components, non-Windows file
  authorities and invalid UTF-8 must not become lookup paths. Double encoding
  is decoded once, and malformed percent escapes remain literal.
- `source-hashes.json` binds the production and regression-test sources.

Reproduce on a host with Dart 3.6.2 installed:

```sh
python3 bench/results/2026-10-19-lsp-uri-paths/check-dart-oracle.py /path/to/dart-sdk /tmp/dart-oracle.json
/path/to/dart-sdk/bin/dart bench/results/2026-10-19-lsp-uri-paths/dart-path-controls.dart
cargo test --locked -p dartforge-lsp --all-targets --target-dir target/impl-a
```

The Rust protocol harness uses its existing synthetic SDK; the official server
capture is independent evidence for these URI scenarios, not full SDK parity.
This validation ran on macOS ARM64; no Windows/Linux execution is claimed for
the path helper. Windows drive/UNC handling remains platform conditional.

## Workspace checks

`checks.json` records commands, exits and hashes of the complete local logs.
Strict LSP Clippy still fails on 16 existing diagnostics (previously 17); no
suppression was added. Workspace formatting and strict Clippy still fail on
existing drift/native diagnostics. Documentation first failed because this
shell lacked an LLVM prefix; with LLVM 22.1.8 explicitly selected, the required
documentation command passed with existing warnings.

The full workspace test run started separately at baseline `81e6762b`, with
the current native compiler and all builder cases enabled. It is still active
in the native builder corpus at this snapshot. That run does not certify this
LSP delta and is not recorded as a pass. Its process and logs are retained in
`/tmp/forge-workspace-configured/`; the compiler executable was not rebuilt
while that run was active.

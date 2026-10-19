# Validate the selected corpus oracles — 2026-10-19

The full workspace run at `81e6762b` ended with Cargo exit **101**. Native
builders passed in 2410.20 s and all four Angular transparency tests passed in
614.15 s, but the differential corpus required the VM to accept every program.
Eight existing JS-interop fixtures explicitly declare that the VM cannot load
their platform libraries. Their selected reference was already DDC, as defined
by `Program::reference_is_ddc`; the corpus gate contradicted that contract.

`baseline-workspace.json` records the terminal result and full log hash;
`baseline-corpus-failure.log` retains the failing test and its report. This
failed workspace run is not reclassified as a success.

## Change and boundaries

`ProgramResult::oracle_failure` is shared by the oracle-only CLI and both corpus
gates. JS validation requires a successful DDC oracle. A nonzero VM result is
accepted only with an existing divergence declaration, exit 254 and a located
CFE diagnostic saying a Dart library is unavailable on the platform. Other VM
failures, missing tools, timeouts and failed DDC execution remain errors.
Native mode still requires a valid VM result; the JS exception does not apply.

Compilation-negative fixtures retain the runners' existing normalization. The
general output/exit-code comparator and Forge result comparison are unchanged.
Oracle-only reports now display `ORACLE!` when DDC fails, so an expected VM
platform rejection cannot conceal a missing web reference.

No program, declaration or oracle comparison was removed. Both oracles still
run for every case. VM platform rejection remains code 254 with its original
diagnostic and remains a DDC×VM divergence. The separate divergence gate still
requires all 14 declared differences to occur, and rejects undeclared ones.

## Validation

- **26 library tests**, **two ignored corpus tests** and **one doctest** pass.
- All **241 programs** have valid DDC oracle results. There are **233 valid VM
  oracle results** (including normalized expected compile errors) and **eight
  declared VM platform rejections**. This is not a claim that all 241 programs
  executed successfully on the VM.
- Negative unit controls cover missing tools, timeouts, runtime and syntax
  errors, malformed CFE locations, undeclared/native rejection, shared oracle
  failures and normalized negative compilation fixtures.
- Actual CLI probes run with the oracle cache disabled: the old web-only
  command exits 1; the corrected command exits 0; native mode still exits 1;
  removing Node from the child PATH still exits 1 and reports the DDC failure.
  Full commands, executable hashes and outputs are in `cli-results.json` and
  the corresponding `.stdout`/`.stderr` files.
- Workspace strict Clippy and warning-denied documentation pass. Global
  formatting still fails. The supported CLI `--features jit` Clippy check also
  passes. Workspace `--all-features` is intentionally invalid because runtime
  `aot` and `dll` profiles are mutually exclusive; that guard was preserved.

The optional Forge runs printed in the corpus report use the explicitly pinned
compiler in `environment.json`. This change validates oracle policy, not a new
compiler binary or a VM/platform execution matrix. Tests ran on macOS ARM64 with
the official Dart 3.6.2 SDK and Node. No newer whole-workspace success is claimed;
the previously documented native foreign-backend pruning requirement remains open.

## Reproduction

From the repository root, with the SDK and Node on PATH:

```sh
cargo test --locked -p dartforge-differential --target-dir target/impl-a -- --include-ignored --nocapture
python3 bench/results/2026-10-19-oracle-validity/check-cli.py --before target/oracle-validation/before-differential --after target/oracle-validation/after-differential --sdk /path/to/dart-sdk --node-dir /path/to/node/bin --out /tmp/oracle-cli-check
```

The CLI probe script was also exercised against the retained binaries; these
ignored build artifacts are not committed. `source-hashes.json` binds the five
changed Rust files. Stored `.log` files trim trailing blank lines only;
`stored-logs.json` records raw/stored hashes. `workspace-checks.json` retains
command results and hashes of the full local check logs.

# Native input and runtime archive observations

Base `b5d6473b`; macOS ARM64, LLVM 22.1.8, object 0.39.1.

The new `native_inputs::inspect` API uses the narrow native/archive readers from
[object](https://docs.rs/object/0.39.1/object/read/index.html), with no write or
compression features. It returns owned observations of relocatable objects and
self-contained archives, without running targets or following external paths.
Whole-input/member/section digests and ranges bind later byte extraction. Member
order and duplicate/non-UTF-8 names survive. Section names/segments, size, alignment,
flags, known image/bitcode candidates and directive inputs remain explicit.

Thin archives, raw bitcode, nested archives, COFF short import members, unknown
formats and non-relocatable images are rejected. A later bad member returns an
error instead of a partial inventory. Mach-O's unified reader tolerates failed
load-command iteration; an explicit fallible walk prevents a truncated command
from appearing as no sections. The regression first confirms the generic parser
accepts that fixture, then confirms the new inspector refuses it. This does not
claim complete object grammar or relocation validation. COFF directive storage,
truncated data and Mach-O linker-option commands have separate regressions.

Marker observations preserve known-address definition, undefined/common state,
weakness, scope and section classification/index. A COFF common marker is neither
a known-address definition nor an undefined reference. Its new regression prevents
an unsafe inference from simply negating is_definition. These are observations,
not complete symbol binding or image authorization.

Final embedded-feature suite: 12 passed, including ignored tests. The default
suite passed 11. The real-object fixture compiles five targets (Mach-O ARM64/x64,
ELF ARM64/x64, COFF x64), confirms one managed section and marker reference, checks
physical bytes/alignment and refuses truncated objects. This reads foreign native
formats on macOS; it does not execute Windows/Linux or inspect their runtimes.
The public API's runnable doctest passed once; an additional zero-test filtered
partition is not counted. Strict workspace/all-target and explicit embedded-llvm
Clippy passed. Warnings-denied private docs passed. Global formatting still fails
existing drift; neither new native_inputs file has a formatting difference.
Full native/workspace suites and desktop/VM/JIT matrices were not repeated.

Each selected local runtime archive (main, DLL, production) has 466 native
members. Main/production have 4,881 sections; DLL has 4,891. No managed-section
candidate, marker symbol, COFF directive section or Mach-O linker-option command
was observed. However, each archive contains 229 known embedded-bitcode section
candidates, totaling 1,331,200 physical bytes. Negative native section findings
alone do not prove the LTO result.

The production archive's member and section ranges/digests were independently
rechecked before all 229 embedded payloads were parsed and verified through the
LLVM inspection API. No explicit managed globals/functions or module assembly
were found there. Five inline assembly sites from compiler_builtins remain
unreviewed: fsqrt/frintn instructions and their complete constraints/flags are
stored as bytes in observations.json. No assembly allowlist was added.

The production archive bytes match the prior SDK publication's runtime digest.
The one added Cargo package nevertheless changes Cargo.lock, an existing input
to runtime source/bundle identity: target bundles must be re-exported for a
compiler rebuilt from this source. Runtime ABI, pruning gates and inventory schema
were not changed. Complete link inputs, member selection, marker binding,
directives, relocations, embedded assembly, LTO and final linked image coverage
remain open. No binary size/speed gain or new image/class authorization is claimed.
No resident process or periodic task was installed.

Text logs normalize trailing whitespace and final blank lines; original/stored
hashes are recorded. fmt-excerpt.log stores only the first 120 lines of the full
failed formatting check, whose original size/hash is retained. Existing LLVM
minimum-deployment link warnings remain in the embedded test log; passing this
round does not certify older macOS compatibility.

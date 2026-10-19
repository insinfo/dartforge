# Verified LLVM bitcode image section observations

Base `7a566b59`; macOS ARM64, LLVM 22.1.8, Dart source SDK 3.6.2.

The new owned-data `dartforge_llvm::inspect_image_sections` API parses IR or
bitcode, verifies the module, and copies explicit global/function sections,
declarations, initializers, assembly and alias/ifunc names. It assigns no managed
class or authorization. Unknown initializer shapes remain `Other`; logical zero
arrays are not expanded, including the u64::MAX regression.

LLVMGetSection supplies no length. The reader recovers complete section bytes
from one canonical module print and checks header counts/names against the C API
lists, including unnamed symbols. This preserves embedded NULs. Printing every
value separately was unnecessarily expensive. The first actual SDK run failed
on a backslash in a constant payload. Clang 22's canonical IR prints that byte
as two backslashes, while other escapes use hexadecimal digits. Both forms are
now decoded; a regression combines quoted symbol/payload bytes, backslashes,
NUL sections and unnamed globals/functions. Alias/ifunc and declaration headers
also have a separate identity regression. Initial failures are preserved here.

The embedded-llvm SDK integration independently reparses stored source image
definitions, reads the actual selected object bytes and checks their publication
digests. Every retained managed-section definition must match the source's name,
complete section, storage properties, alignment, address space and initializer.
Optimizer deletion and visibility tightening are allowed, additions or changed
retained blocks are refused. Functions in image sections, replacement of known
image symbols by aliases/ifuncs and unreviewed assembly are refused.

All 15 selected SDK artifacts passed: 5,255 source image definitions, 5,233
retained, 22 removed. There were 31,395 inline assembly sites after inlining,
all the exact reviewed macOS ARM64 TSD instruction. The inventory and all module
records are identical to the previous schema-5 snapshot; the new API/test does
not change emitted IR, runtime ABI or authorization. The cache file remains
69,945,429 bytes. Digests and per-artifact counts are stored separately.

64 inventory tests including the ignored actual SDK integration passed. Eight
image inspection tests, the five-target object emission test and one runnable
public API doctest passed. Strict workspace/all-target Clippy and explicit
embedded-llvm Clippy and warnings-denied private docs passed. Global formatting
still fails existing drift;
the newly added inspection files and integration call/module declarations have
no formatting diff after their final formatting adjustment. Full native/workspace
suites, production-profile assertions and desktop/VM/JIT matrices were not repeated.

These SDK `.o` files are LLVM bitcode (with Mach-O wrappers), not final native
images. The parser's target-format regressions read bitcode for Mach-O/ELF/COFF;
they do not execute those targets. This integration round inspected selected
macOS artifacts. Runtime archives, foreign link inputs, marker binding,
relocations, comdat merging, linker options, post-LTO output and complete SDK
startup authorization remain open. No binary size/speed reduction is claimed.
No resident process or periodic task was installed.

Text logs normalize trailing whitespace; original/stored hashes are recorded.
The final runtime suite preceded whitespace-only formatting of its integration
call and module declarations. The complete API's eight-test run and strict
feature Clippy cover the final implementation.

`fmt-excerpt.log` stores only the first 120 lines of the failed global formatting
check; `log-hashes.json` binds the complete original output.

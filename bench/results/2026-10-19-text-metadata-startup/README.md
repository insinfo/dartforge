# Certify exact text metadata in startup, separately from managed images

Base `c7901f8b`; macOS ARM64, LLVM 22.1.8, Dart source SDK 3.6.2.

Inventory schema 3 stores native_initializer for literal immutable byte arrays,
and explicit null for other globals. The initializer is checked against its
recorded digest and symbol, decoded with LLVM hex escapes and an exact array
length, and accepted for text metadata only when uniquely private to the module.
Simple alignment and comments outside quotes are supported; unsupported forms
stay opaque. Prior schema snapshots do not supply absent initializers.

The entry-only registry certificate now composes register_class_name and
register_argument_name with nominal subtype publication. Each API requires the
exact ABI, a direct pointer to a checked private byte constant, exact full-array
length and valid UTF-8. Class names additionally require valid local nominal CIDs;
conflicting names for one CID within the startup are rejected. Other effects,
managed images, method tables, helper bindings and ordinary calls remain outside
this certificate. The runtime bodies copy text into host maps, not Dart objects.
Future native consumers of these maps still need their own coverage contracts.

The temporary proof graph discharges only certified metadata API edges and
spurious address citations from checked literal byte globals (such as an @ inside
text). Original summaries and executable bodies remain intact. Generic import
calls are not authorized to return addresses or managed references by these facts.

Tests cover mixed and separate text APIs, UTF-8/escaped quotes/backslashes/semicolons,
array lengths, malformed byte initializers, invalid UTF-8, public/duplicate data,
negative/inexact lengths, offset pointers, unknown CIDs and helper effects.
Re-signed records with missing/modified initializers, wrong symbols or inconsistent
lengths fail semantic loading even after publication integrity passes. A runtime
unit test verifies exact stored text and unchanged Dart allocation counters. The
compiled broad/certified startup includes names and subtype edges, executes with
normal/stress GC, and checks transitive/inverse nominal queries. It is not a full
reflection, selector-error or JIT ABI test.

Final validation: 52 inventory tests, including actual production SDK rebuild,
publication and loading; 51 class-proof tests; 14 general pruning tests; and the
runtime contract test passed. Strict all-target workspace Clippy and private docs
with warnings denied pass. Global fmt still fails existing drift. Full native/
workspace suites, the production-profile assertion and desktop/VM/JIT matrices
were not repeated.

The selected SDK inventory contains 7,753 retained native initializers. Comparing
schema 2 and 3 snapshots found identical final-IR digests for all modules and
unchanged digests for all 15 registration startups. Compiler cache JSON grew
66,874,532 -> 67,760,874 bytes (about 1.3%). This is cache metadata cost, not an
executable-size or performance measurement. Comparison paths and hashes are kept;
large cache inventories are not committed.

The real SDK test confirms its complete startups remain unadmitted: managed
images, method tables and helpers still need independent certificates. Complete
backend pruning and executable size/performance improvements remain unproved.
The serialized schema changed, not the runtime ABI or .poda format. Runtime
production function bodies are unchanged; its source edit adds a contract test.

Text logs normalize trailing whitespace and retain original/stored digests.
No resident process or scheduled task was installed.

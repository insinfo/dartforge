# DartForge

Optional DartForge capabilities and portable SIMD extensions in one package.
Source stays valid on the official Dart SDK. The package is local and is not
published to pub.dev.

Import the public API with `package:dartforge/dartforge.dart`. Compilation
identity (`isDartForge`) is separate from individual runtime capabilities;
check the capability before selecting a platform-specific operation.

## Optimization annotations

The annotation API has `dartForgeAnnotationsSchema == 1`. Annotations are
exported by the root API and by `package:dartforge/annotations.dart`; both
entry points expose the same declarations and constant identities:


| Annotation | Intended target | Request |
| --- | --- | --- |
| `PreferRegion` | Synchronous function or method with a body | Prefer region analysis for confined allocations. |
| `PreferAcyclic` | Class, constructor, or local variable declaration | Prefer proof that associated allocation sites do not participate in cycles. |
| `NoEscape` | Formal parameter of a function or method with an available body or certified summary | Prove that the call creates no persistent escape of the parameter or references derived from it. |
| `Realtime` | Synchronous function or method with a body | Request the proposed transitive, bounded `dsp-v1` execution contract. |

```dart
import 'package:dartforge/dartforge.dart';

@PreferAcyclic()
final class Sample {
  @PreferAcyclic()
  const Sample(this.value);
  final double value;
}

@PreferRegion()
double read(@NoEscape() Sample sample) => sample.value;

@Realtime()
double gain(double value, double factor) => value * factor;

void main() {
  @PreferAcyclic()
  const sample = Sample(2);
  print(gain(read(sample), 3)); // 6.0
}
```

These are ordinary const metadata. **Recognition does not produce optimization
facts or certify `NoEscape`/`Realtime`.** The native compiler resolves the
actual declarations and constant aliases, validates the schema and targets,
and keeps preferences separate from verification requests.
The example demonstrates portable Dart syntax and behavior; it is not a
certificate or a measured optimization. Adding metadata does not change the
Dart VM's memory policy or execute code when an annotated function is entered.

Preferences cannot force stack allocation, classify all subclasses or reachable
objects as acyclic, weaken references, or remove safety checks. `NoEscape` does
not transfer ownership or promise absence of allocation, mutation, exceptions,
or blocking. `Realtime` is intended to verify the complete execution path,
including callees, generated wrappers, cleanup, and runtime entry; annotating a
method alone provides no device-entry safety or operating-system deadline.
There is no audio entry or complete contract proof checker installed by these
declarations. An internal conservative HIR non-publication analysis is under
development. Its admitted bodies now require typed CFG/SSA validation, including
dominance, exact call types and Phi edge accounting. Results still require
source-to-ABI parameter mapping, native scalar alias provenance, generated
helper/cleanup audits and rechecking after transformations. They do not discharge
metadata obligations or change ownership decisions.

Native compilation defaults to `--dartforge-contracts=verify`. Known
`NoEscape` and `Realtime` requests currently fail with positioned diagnostics
because complete emission-level proof checkers are not implemented. Preferences compile and are
reported as `not_applied`. To compile the ordinary Dart behavior explicitly,
use `--dartforge-contracts=ignore`; the report records ignored requests and
metadata diagnostics without producing safety facts. Dart language errors
remain errors in either mode.

Successful native builds write an adjacent `.contracts.json` report with mode,
source/package fingerprints and artifact identity. IR emission also writes
provenance beside its output. Reports currently have `certified: false`;
policy and dependency changes participate in the object-cache input.

`package:meta` supplies analyzer target feedback for functions, methods, and
parameters. It cannot enforce synchronous bodies, body availability, or effects.
It has no local-variable target, so `PreferAcyclic` deliberately has no `Target`
restriction and its intended targets are documented above. Compiler
validation resolves the actual declarations in
`package:dartforge/src/annotations.dart`, including through public exports,
prefixes, reexports, and const aliases. A matching class name is not authority.

## Resource scopes and library loading

`using(resource, body)` disposes a `ScopedResource` in `finally`. The body must
finish synchronously and must not retain the resource. `usingAsync` awaits a
synchronous result or a future before disposing. Cleanup is synchronous in both
APIs. A cleanup error replaces a pending body error, following ordinary Dart
`try/finally` behavior. `ResourceHandle<T>` adapts an existing resource through
a disposal callback and attempts disposal at most once. These helpers supply
portable scope cleanup, without ARC semantics or a compiler escape proof.
Resource implementations and disposal callbacks must also finish synchronously;
the helpers do not detect async-void callbacks or await asynchronous cleanup.

`NativeLibrary.openFirst(candidates)` opens the first loadable path in order.
Its `selectedPath` records the winner; if all candidates fail,
`NativeLibraryOpenError.failures` preserves each original loader error and
path. `hasSymbol` checks availability, `lookup<T>` returns a pointer, and
`library` borrows the SDK handle for `lookupFunction`. Close the owner only
after calls and callbacks using its symbols have finished. Its `dispose()`
integrates with resource scopes, while `close()` is idempotent. Queries after
close throw; previously obtained pointers/functions must no longer be used.

```dart
import 'package:dartforge/dartforge.dart';

void main() {
  // macOS example; pass the appropriate platform library candidates.
  final library = NativeLibrary.openFirst(['/usr/lib/libSystem.B.dylib']);
  print(using(library, (owner) => owner.hasSymbol('strlen')));
}
```

These library APIs use the existing `DynamicLibrary` implementation on both
runtimes. They do not statically link `@Native` symbols or change native ABIs.

## Mapped files

`MappedMemory.open(path, offset: ..., length: ..., writable: ...)` maps an
existing file through POSIX `mmap` or Windows `MapViewOfFile`. The portable
implementation supports 64-bit macOS, Linux and Windows. It checks ranges,
aligns native offsets, does not grow the file, and permits empty ranges.
`bytes` and `view()` access the mapping without copying through a checked
`MappedBytes` facade; retained facades and iterators reject access after close.
`toList` and `sublist` produce owned copies.

`withBytes` borrows an external `Uint8List` for a synchronous callback, providing
an unmodifiable view for read-only mappings. The callback must not retain or
return the view, its buffer or derived views, and must not run asynchronously.
These are caller obligations, not compiler escape proofs. Close is rejected
while the callback is active, but an escaped raw view cannot be revoked and
must never be accessed after the mapping closes. Use the checked facade when
revocable access is needed.

```dart
import 'package:dartforge/dartforge.dart';

void main() {
  final mapping = MappedMemory.open('data.bin');
  print(using(mapping, (owner) => owner.withBytes((bytes) => bytes.length)));
}
```

Mappings belong to their creating isolate and require explicit disposal.
Writable mappings support synchronous `flush`; concurrent writers require
external synchronization, and callers must prevent file truncation during the
mapping lifetime. OS faults caused by truncation or storage failures are not
converted into Dart errors. Windows/Linux implementations still require their
own platform execution evidence; macOS validation does not establish that.

## Native waiting

`EventLoop.supportsNativePump` reports an optional macOS process-main-thread
CFRunLoop adapter. The official VM and other platforms report false. This
capability is separate from compiler identity and from annotation metadata.

```dart
import 'package:dartforge/dartforge.dart';

Future<void> main() async {
  if (!EventLoop.supportsNativePump) return;
  EventLoop.enableNativePump();
  try {
    await Future<void>.delayed(const Duration(milliseconds: 10));
  } finally {
    EventLoop.disableNativePump();
  }
}
```

Enabling keeps the application alive until disabled. Enable/disable are
idempotent application lifecycle operations, not reference-counted scopes.
Native waiting delivers CFRunLoop sources and main-queue work between Dart
events; applications still dispatch their AppKit NSEvents. This API neither
executes nested Dart turns nor enables synchronous foreign-thread callbacks.
Long callbacks, microtask drains and native batches have no latency bound.
Disabling removes the adapter; it does not cancel application-owned native
timers or queued callbacks. Applications must finish those callbacks before
closing their callable contexts.

`test/native_pump_probe.dart` checks native main-queue delivery, timer/microtask
wakeups, worker messages and owner checks. `test/native_pump_lifecycle_probe.dart`
checks repeated installation and shutdown from a callback without a Dart
timer or port keeping the application alive. On the VM both check the
unsupported fallback. Compile these probes with the native-enabled DartForge
CLI and the package's resolved `.dart_tool/package_config.json`; run normal
and GC-stress variants separately.

## Closed native callbacks in resident AOT code

DartForge AOT preserves published `NativeCallable` trampoline addresses after
`close()` and when their owning isolate terminates. A later call returns the
configured scalar `exceptionalReturn`, drops a void/listener notification, or
returns a zeroed aggregate. The closed path runs before Dart argument decoding
and isolate entry. Closing releases the registered Dart closure root; an already
active local callback retains its own frame until it returns.

This is a DartForge runtime extension. The Dart VM's behavior after `close()`
remains undefined: never test late invocation on the official VM. Live
`isolateLocal` callbacks still require their owning thread. This extension does
not enable synchronous foreign-thread callbacks or certify realtime audio.

Published executable slots and native callback contexts remain allocated for the
process lifetime and are never reused. Memory therefore grows with the number
of callbacks created, including closed callbacks. The guarantee requires
process-resident AOT callback entry code; JIT module unloading/reload has not
been validated. Stop the native producer before unloading its libraries or
releasing argument storage: a tombstone cannot extend those separate lifetimes.

## Native audio callbacks

The package currently has no prepared AOT entry for running Dart audio kernels
synchronously on a foreign device thread. `Realtime` metadata and native-pump
support do not provide that entry or certify an audio callback.

`NativeCallable.isolateLocal` must be called on its owning isolate's thread;
calling it from another thread aborts the process. `NativeCallable.listener`
accepts calls from other threads but delivers them asynchronously to the
owner, returns only `void`, and requires native resources to remain valid
until the listener finishes. It cannot fill an audio device's output buffer
synchronously before the device callback returns. A proposed
`SyncNativeCallable` that waits for the owner would introduce scheduling and
blocking costs; waiting is incompatible with the proposed `dsp-v1` contract.
It is not an available audio API in this package.

A future bridge must separate four phases:

1. **Prepare:** resolve and validate the AOT entry, allocate buffers and context,
   establish their owner/domain and prepare thread, stack and runtime state
   before device callbacks begin.
2. **Process:** run only a verified, bounded kernel over valid preallocated
   storage, without allocation, release cascades, collection, locks, logging,
   lazy initialization or access to another isolate's heap.
3. **Quiesce:** stop new entries and establish that every in-flight callback
   has returned, without making the device thread wait for shutdown or updates.
4. **Dispose:** release buffers, context, code registrations and native resources
   on the owning control domain after quiescence.

The first bounded implementation should target a scalar/numeric-buffer kernel
with an explicit native ABI and audited wrapper. General Dart closures,
managed objects and callbacks that enter an arbitrary isolate need additional
protocols. Certification must cover the entry and its complete call graph,
including stack/safepoints, ownership and cleanup; a kernel-only result is
not an audio-device entry certificate. See the proposed profile and lifecycle
in [the ARC specification, section 36](../../docs/ARC-CICLOS-ESPECIFICACAO.md).

## SIMD extensions

`package:dartforge/simd.dart` exports the portable `Int32x4` extensions;
`package:dartforge/dartforge.dart` exports the same declarations. There is one
implementation in `lib/src/simd/int32x4_3_14.dart`. Importing both public entry
points does not introduce competing extensions. The `test/simd_probe.dart`
probe checks both exports, lane overflow, masked shifts, comparisons and masks.

## Validation

With Dart 3.6.2 and dependencies resolved:

```sh
dart analyze
dart run test/annotations_probe.dart
dart run test/simd_probe.dart
python3 test/verify_annotation_targets.py
python3 test/verify_compiler_contracts.py
```

From the repository root, with a native-enabled compiler already built:

```sh
python3 scripts/verify-package-resources.py \
  --probe native_library_probe.dart \
  --probe resources_probe.dart \
  --probe mapped_memory_probe.dart \
  --probe mapped_memory_isolate_probe.dart \
  --output target/package-resources
```

The resource checks compare VM, official AOT and six native variants per probe
(tracing, ARC and prepared ARC, normal and GC stress). They record source and
artifact fingerprints and reject input changes during the run. They establish
functional behavior on the executing platform, not throughput or latency gains.

The probe executes the example, imported aliases, and constant identity checks.
The target check creates invalid fixtures in a temporary directory, verifies
analyzer diagnostics, and removes them. These checks establish source
portability and target feedback; they do not prove compiler contracts.

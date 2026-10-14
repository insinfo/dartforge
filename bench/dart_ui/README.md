# dart_ui differential kernels

These probes import the current framework implementation. They do not vendor
or replace its algorithms. Use a `main` checkout with an existing
`.dart_tool/package_config.json`; the runner neither edits it nor runs `pub get`.

```sh
source /Volumes/ssd_dock/tools/env.zsh
export DARTFORGE_CLANG=/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang
python3 scripts/verify-dart-ui.py \
  --dart-ui-root /Volumes/ssd_dock/MyProjectsDart/dart_ui \
  --output target/dart-ui-compat
```

The runner compares VM JIT and official AOT output, then checks native AOT
with tracing, ARC and prepared ARC, each normally and with GC stress. Native
builds use optimization, table exceptions and shadow roots; effects and ARC
checks remain enabled. `--probe video|strip|ffi` and `--mode tracing|arc|prepared`
select a smaller run. Build logs and exact output files remain in the supplied
output directory; `validation.json` records source/tool hashes and flags.

`video_probe.dart` checks real conversion bytes, plane offsets/strides,
color space/range, crop, reused destinations and packed pixel formats.
`strip_probe.dart` checks complete pixels and active packed records for eleven
geometry/fill/clip fixtures, including winding, overlap and buffer reuse.
`ffi_probe.dart` exercises the framework's allocator, arena, views, mutex,
struct layouts and callback lifecycle, including exception handling.
These fixtures do not establish full framework compatibility.

The default mode validates correctness and prints no benchmark timings.
The video probe accepts `--benchmark N [width height [pixelFormat]]`, with
NV12, I420, YUY2, RGBA or BGRA input. It reuses the output and reports conversion
time separately from the complete output checksum. Converter-internal
allocations remain part of the measured work.
The strip probe separately accepts `--benchmark N` to time warmed rasterization plus a checksum of all active output
with buffer growth recorded. Run performance experiments after builds finish,
with identical inputs and repeated isolated runs; smoke timings are not evidence
of a speed improvement.

## Embedded AppKit capability probe

`embedded_probe.dart` imports the current `MacosEmbeddedWindowingBackend` and
`MacosEmbeddedRuntime`. It checks all five runtime classifications through the
backend's existing `runtimeStatus` hook, then verifies that inactive
`initialize()` throws before AppKit is touched. The injected active status is
used only to inspect capabilities. It does not authorize initialization.
On the stock runtime, the probe also checks that the real
`requestRunApplication()` API rejects its missing patched-runtime symbol.
No application, window, signal transport or replacement `Dart_*` export is
created. Use the same package configuration as the other framework probes.
This checks compilation and capability guards, not a working application loop.

The current integration points need a loop adapter before real initialization:

- `embedded_backend.dart` now accepts an optional `requestRunApplication`
  callback alongside its status hook. The default remains the patched SDK
  runtime implementation. A supplied adapter must verify the main thread and
  the specific supported pump/dispatch capabilities before reporting active.
- Initialization awaits `applicationDidFinishLaunching`. A DartForge path can
  reuse real Cocoa calls to `finishLaunching` after delegates are installed,
  then dispatch AppKit events in bounded batches with
  `nextEventMatchingMask:untilDate:inMode:dequeue:` and `sendEvent:`. Existing
  `macos_backend.dart` and `appkit_signal_session.dart` contain these patterns;
  their signal transport and sleep/poll loops should not be copied.
- The runtime needs a safe dispatch boundary after native wake and between Dart
  events. CFRunLoop integration alone does not dequeue `NSEvent`. Calling
  `[NSApp run]` blocks this runtime's Dart scheduler, and modal dispatch can
  also block; neither path currently proves nested-turn support. Dispatch must
  happen without queue locks or borrowed heap/event state and preserve ordinary
  Dart microtask/event ordering.
- Initialization now cleans partial delegates, retained objects, the callback
  sink and kick port on Dart-level failure. `shutdown()` also handles allocated
  partial state before `_initialized` becomes true. The adapter must stop its
  dispatcher and release pump ownership without closing callbacks that AppKit
  still retains.

The safe probe currently compiles and runs; its observed blocker is the absent
patched run-application bridge, not an unsupported Dart syntax feature. Passing
these checks does not show that the positive AppKit initialization branch,
input, menus, resize or shutdown after partial launch works.

### Finite application integration

Run the separate live application check with existing framework and DartForge
package configurations; it does not run `pub get` or edit the framework:

```sh
python3 scripts/verify-embedded-integration.py \
  --framework /Volumes/ssd_dock/MyProjectsDart/dart_ui \
  --compiler target/p4/release/dartforge \
  --output target/embedded-integration-final
```

The macOS arm64 runner checks the upstream unsupported fallbacks separately,
then all six native memory/stress combinations against the explicit expected
launch/cleanup/event trace. Its ledger records source, framework patch, compiler
and binary hashes. Each binary has a distinct full-filename
`.contracts.json` sidecar bound to its artifact path; these unannotated probes
do not claim a `NoEscape` or `Realtime` certificate.


`embedded_integration_probe.dart` is a separate capability-gated application
probe. The official SDK prints an unsupported-capability fallback; its output
is not a pixel/byte oracle for the positive native path. It uses the new
`EventLoop` dispatcher on outer Dart turns, not a replacement SDK export.
The live host is strongly referenced across awaited operations.

The native path installs real embedded AppKit classes/delegates. An injected
startup failure checks original exception identity, cleared delegate/sink and
idempotent shutdown before a fresh backend is initialized. The startup adapter
then calls `finishLaunching`, waits for the real launch notification and
handles a synthetic `NSEvent`, a one-shot timer and an isolate message. It
returns after shutdown without creating a window. Each dispatch batch handles
at most 32 events; a full batch schedules a Dart event for further work.

This requires a CLI built with native-event-dispatch support. It does not
certify window input, drawing, native resize/menu loops or initialization
failure modes outside the injected Dart exception. Positive AppKit dispatch is
not performed on the VM or official AOT fallback.

### Finite real-window lifecycle

```sh
python3 scripts/verify-embedded-window.py \
  --framework /Volumes/ssd_dock/MyProjectsDart/dart_ui \
  --compiler target/p4/release/dartforge \
  --output target/embedded-window-final
```

`embedded_window_probe.dart` reuses the bounded application dispatcher and
creates an actual embedded `NSWindow`, initially hidden. It verifies framework
and native identities, the backend registry, showing the window, programmatic
client resize and its event/surface-generation change. A synthetic application
event, a timer and an isolate message must progress while the window exists.
Calling AppKit's `performClose:` must produce the framework close request;
explicit, idempotent framework close must produce exactly one closed event,
invalidate the generation and remove the window from the backend registry.
The probe also checks cleanup after an injected startup exception before the
successful launch. The visible window closes automatically, including on
Dart-level assertion failures.

The runner checks VM and official AOT unsupported fallbacks separately from the
positive native oracle, then validates tracing, ARC and prepared ARC with and
without GC stress. Its ledger binds the source, imported dispatcher helper,
framework sources/patch, compiler, binaries and distinct contract sidecars.
This checks window lifecycle and outer-turn scheduler progress; it does not
measure performance or verify drawing, user input, menus, live resize, modal
loops or nested Dart turns.

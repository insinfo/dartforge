/// Imports the real appkitEmbedded backend without creating an application or
/// window. Status injection exercises its existing capability API, and an
/// inactive initialize call verifies rejection before any AppKit operation.
/// No symbols belonging to the patched Dart SDK are synthesized or exported.
import 'package:dart_ui/src/backends/macos/embedded/embedded_backend.dart';
import 'package:dart_ui/src/backends/macos/embedded/embedded_runtime.dart';
import 'package:dart_ui/src/foundation/diagnostics.dart';

void require(bool condition, String message) {
  if (!condition) throw StateError(message);
}

Future<void> main() async {
  final statuses = <MacosEmbeddedRuntimeStatus>[
    classifyMacosEmbeddedRuntime(
        isMacOS: false,
        hasLoopSymbol: false,
        loopValue: 0,
        onMainThread: false),
    classifyMacosEmbeddedRuntime(
        isMacOS: true, hasLoopSymbol: false, loopValue: 0, onMainThread: true),
    classifyMacosEmbeddedRuntime(
        isMacOS: true, hasLoopSymbol: true, loopValue: 0, onMainThread: true),
    classifyMacosEmbeddedRuntime(
        isMacOS: true, hasLoopSymbol: true, loopValue: 1, onMainThread: false),
    classifyMacosEmbeddedRuntime(
        isMacOS: true, hasLoopSymbol: true, loopValue: 1, onMainThread: true),
  ];
  const expected = <MacosEmbeddedRuntimeStatus>[
    MacosEmbeddedRuntimeStatus.notMacos,
    MacosEmbeddedRuntimeStatus.stockRuntime,
    MacosEmbeddedRuntimeStatus.loopOff,
    MacosEmbeddedRuntimeStatus.notMainThread,
    MacosEmbeddedRuntimeStatus.active,
  ];
  for (var i = 0; i < statuses.length; i++) {
    final status = statuses[i];
    require(status == expected[i], 'embedded runtime classification');
    final backend = MacosEmbeddedWindowingBackend(
        runtimeStatus: () => status, applicationName: 'DartForge probe');
    final result = backend.probe();
    require(result.supported == status.isActive, 'backend capability status');
    print(
        'status=${status.name} supported=${result.supported} backend=${backend.name}');
    // The active hook is used only for capability inspection. It does not
    // establish a real runtime loop and must not authorize AppKit initialization.
    await backend.shutdown();
  }
  final inactive = MacosEmbeddedWindowingBackend(
      runtimeStatus: () => MacosEmbeddedRuntimeStatus.stockRuntime);
  var rejected = false;
  try {
    await inactive.initialize();
  } on UnsupportedCapabilityError catch (error) {
    require(error.capability == Capability.window, 'initialize rejection kind');
    rejected = true;
  } finally {
    await inactive.shutdown();
  }
  require(rejected, 'inactive backend must reject initialize');
  print('inactive initialize rejected before AppKit');
  final processStatus = MacosEmbeddedRuntime.status;
  print('process runtime=${processStatus.name}');
  if (processStatus == MacosEmbeddedRuntimeStatus.stockRuntime) {
    var bridgeMissing = false;
    try {
      MacosEmbeddedRuntime.requestRunApplication();
    } on ArgumentError {
      bridgeMissing = true;
    }
    require(bridgeMissing, 'stock runtime must not provide the patched bridge');
    print('patched run-application bridge unavailable');
  }
  print('embedded capability fixtures verified; no window or application loop');
}

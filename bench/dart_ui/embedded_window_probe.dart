/// Finite real AppKit window lifecycle on outer Dart event turns.
/// Official runtimes exercise only the unsupported capability fallback.
import 'dart:async';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';

import 'package:dartforge/dartforge.dart';
import 'package:dart_ui/src/backends/macos/embedded/embedded_appkit.dart';
import 'package:dart_ui/src/backends/macos/embedded/embedded_backend.dart';
import 'package:dart_ui/src/backends/macos/embedded/embedded_runtime.dart';
import 'package:dart_ui/src/backends/macos/embedded/embedded_window.dart';
import 'package:dart_ui/src/ffi/objc_runtime.dart';
import 'package:dart_ui/src/geometry/offset.dart';
import 'package:dart_ui/src/geometry/rect.dart';
import 'package:dart_ui/src/geometry/size.dart';
import 'package:dart_ui/src/platform/native_window.dart';
import 'package:dart_ui/src/platform/window_events.dart';

import 'embedded_integration_probe.dart' show AppKitDispatcher, require;

Future<void> main() async {
  if (!Platform.isMacOS ||
      !EventLoop.supportsNativePump ||
      !EventLoop.supportsNativeEventDispatcher) {
    print('embedded window unsupported: native pump/dispatcher unavailable');
    return;
  }
  require(DynamicLibrary.process().lookupFunction<Int32 Function(),
          int Function()>('pthread_main_np')() == 1,
      'AppKit requires the process main thread');
  final host = AppKitDispatcher();
  final backend = MacosEmbeddedWindowingBackend(
      runtimeStatus: () => MacosEmbeddedRuntimeStatus.active,
      requestRunApplication: host.startApplication,
      applicationName: 'DartForge finite window probe');
  MacosEmbeddedWindow? window;
  StreamSubscription<PlatformWindowEvent>? events;
  Timer? timer;
  RawReceivePort? port;
  EventLoop.enableNativePump();
  try {
    final injected = StateError('injected window probe launch failure');
    final failed = MacosEmbeddedWindowingBackend(
        runtimeStatus: () => MacosEmbeddedRuntimeStatus.active,
        requestRunApplication: () { throw injected; });
    var preserved = false;
    try { await failed.initialize(); }
    on StateError catch (error) { preserved = identical(error, injected); }
    require(preserved && MacosEmbeddedClasses.sink == null,
        'failed initialization preserves error and clears callback sink');
    ObjCAutoreleasePool.run(() {
      final app = objcSendPointer(
          objcClass('NSApplication'), objcSelector('sharedApplication'));
      require(objcSendPointer(app, objcSelector('delegate')) == nullptr,
          'failed initialization clears native delegate');
    });
    await failed.shutdown();
    print('window launch failure cleanup verified');

    await backend.initialize();
    require(backend.launchDuration != null, 'real launch notification');
    final created = await backend.createWindow(const WindowOptions(
        size: Size(160, 100), position: Offset(48, 48),
        title: 'DartForge finite window probe', visible: false));
    require(created is MacosEmbeddedWindow, 'real embedded window type');
    final live = created as MacosEmbeddedWindow;
    window = live;
    final generation = live.generation;
    require(live.id.value > 0 && live.nativeWindow != nullptr &&
        live.view != nullptr && live.windowNumber > 0,
        'framework and native window identities');
    require(backend.windows.length == 1 &&
        identical(backend.windowFor(live.nativeWindow), live) &&
        identical(backend.windowFor(live.view), live), 'window registration');
    require(objcSendUnsigned(live.nativeWindow, objcSelector('isVisible')) == 0,
        'hidden creation');
    require(live.clientSize == const Size(160, 100), 'initial client size');
    final resized = Completer<void>();
    final requested = Completer<void>();
    final closed = Completer<void>();
    var closeCount = 0;
    var requestCount = 0;
    events = live.events.listen((event) {
      require(event.windowId == live.id, 'window event identity');
      if (event is WindowResizedEvent &&
          event.clientSize == const Size(192, 120)) {
        require(event.generation > generation &&
            event.generation == live.generation && event.renderScale > 0,
            'resize generation ${event.generation}/$generation scale ${event.renderScale}');
        if (!resized.isCompleted) resized.complete();
      }
      if (event is WindowCloseRequestedEvent) {
        require(event.generation == live.generation, 'close request generation');
        requestCount++;
        if (!requested.isCompleted) requested.complete();
      }
      if (event is WindowClosedEvent) {
        require(event.generation == live.generation &&
            event.generation > generation, 'closed generation matches disposed window');
        closeCount++;
        if (!closed.isCompleted) closed.complete();
      }
    });
    live.show();
    require(objcSendUnsigned(live.nativeWindow, objcSelector('isVisible')) != 0,
        'native window shown');
    live.setBounds(const Rect.fromLTWH(48, 48, 192, 120));
    require(live.clientSize == const Size(192, 120) &&
        live.generation > generation, 'resize invalidates surface generation');
    final tick = Completer<void>();
    final message = Completer<void>();
    port = RawReceivePort((Object? value) {
      require(value == 37, 'window probe isolate message');
      if (!message.isCompleted) message.complete();
    });
    timer = Timer(const Duration(milliseconds: 10), () => tick.complete());
    port.sendPort.send(37);
    host.postSyntheticEvent();
    await Future.wait<void>([resized.future, tick.future,
        message.future, host.synthetic.future]).timeout(const Duration(seconds: 5));
    require(host.syntheticCount == 1, 'one synthetic application event');
    print('window registered hidden then shown; resize=192x120 event verified');
    print('window dispatch synthetic=1 message=37 timer=1');
    ObjCAutoreleasePool.run(() => objcSendVoid1(
        live.nativeWindow, objcSelector('performClose:'), 0));
    await requested.future.timeout(const Duration(seconds: 5));
    require(!live.isDisposed && requestCount == 1,
        'delegate requests close without premature disposal');
    final beforeCloseGeneration = live.generation;
    live.close();
    live.close();
    require(live.generation > beforeCloseGeneration,
        'close invalidates the current surface generation');
    await closed.future.timeout(const Duration(seconds: 5));
    require(live.isDisposed && closeCount == 1 && backend.windows.isEmpty,
        'idempotent close and backend removal');
    require(backend.windowFor(live.nativeWindow) == null &&
        backend.windowFor(live.view) == null, 'closed window mapping removed');
    print('window close request=1 closed=1 generation invalidated registry empty');
  } finally {
    timer?.cancel();
    port?.close();
    window?.close();
    await events?.cancel();
    await backend.shutdown();
    host.dispose();
    EventLoop.disableNativePump();
  }
  require(MacosEmbeddedClasses.sink == null, 'shutdown callback sink');
  print('embedded window shutdown verified; no rendering or modal loop');
}

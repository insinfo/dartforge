/// Finite AppKit launch/event-dispatch integration through the real embedded
/// backend. No window is created. Modal loops and nested Dart turns are outside
/// this probe. The official SDK takes the unsupported-capability fallback.
import 'dart:async';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';

import 'package:dartforge/dartforge.dart';
import 'package:dart_ui/src/backends/apple/cocoa_ffi.dart';
import 'package:dart_ui/src/backends/macos/embedded/embedded_backend.dart';
import 'package:dart_ui/src/backends/macos/embedded/embedded_appkit.dart';
import 'package:dart_ui/src/backends/macos/embedded/embedded_runtime.dart';
import 'package:dart_ui/src/ffi/objc_runtime.dart';

typedef EventNative = Pointer<ObjCObject> Function(
    Pointer<ObjCObject>,
    Pointer<ObjCSelector>,
    UintPtr,
    CGPoint,
    UintPtr,
    Double,
    IntPtr,
    Pointer<ObjCObject>,
    Int16,
    IntPtr,
    IntPtr);
typedef EventDart = Pointer<ObjCObject> Function(
    Pointer<ObjCObject>,
    Pointer<ObjCSelector>,
    int,
    CGPoint,
    int,
    double,
    int,
    Pointer<ObjCObject>,
    int,
    int,
    int);

void require(bool condition, String message) {
  if (!condition) throw StateError(message);
}

final class AppKitDispatcher {
  Pointer<ObjCObject> application = nullptr;
  Pointer<ObjCObject> distantPast = nullptr;
  int defaultMode = 0;
  bool active = false;
  int syntheticCount = 0;
  final Completer<void> synthetic = Completer<void>();

  void startApplication() {
    ObjCAutoreleasePool.run(() {
      application = objcRetain(objcSendPointer(
          objcClass('NSApplication'), objcSelector('sharedApplication')));
      distantPast = objcRetain(
          objcSendPointer(objcClass('NSDate'), objcSelector('distantPast')));
      defaultMode = DynamicLibrary.open(
              '/System/Library/Frameworks/Foundation.framework/Foundation')
          .lookup<IntPtr>('NSDefaultRunLoopMode')
          .value;
      active = true;
      EventLoop.setNativeEventDispatcher(dispatch);
      // This invokes the real registered application delegate. It does not
      // enter NSApp.run or block Dart's scheduler behind an AppKit loop.
      objcSendVoid(application, objcSelector('finishLaunching'));
    });
  }

  void postSyntheticEvent() {
    ObjCAutoreleasePool.run(() {
      final make = objcSendTrampoline
          .cast<NativeFunction<EventNative>>()
          .asFunction<EventDart>();
      final event = make(
          objcClass('NSEvent'),
          objcSelector(
              'otherEventWithType:location:modifierFlags:timestamp:windowNumber:context:subtype:data1:data2:'),
          15,
          cgPoint(0, 0),
          0,
          0,
          0,
          nullptr,
          1078,
          73,
          91);
      require(event != nullptr, 'synthetic NSEvent allocation');
      objcSendVoid2(
          application, objcSelector('postEvent:atStart:'), event.address, 0);
    });
  }

  void dispatch() {
    if (!active) return;
    // Every batch returns to the outer scheduler. A full batch queues a Dart
    // event rather than draining recursively or entering a nested native wait.
    var count = 0;
    while (count < 32 && active) {
      final more = ObjCAutoreleasePool.run(() {
        final event = objcSendPointer4(
            application,
            objcSelector('nextEventMatchingMask:untilDate:inMode:dequeue:'),
            -1,
            distantPast.address,
            defaultMode,
            1);
        if (event == nullptr) return false;
        if (objcSendUnsigned(event, objcSelector('type')) == 15 &&
            objcSendUnsigned(event, objcSelector('data1')) == 73 &&
            objcSendUnsigned(event, objcSelector('data2')) == 91) {
          syntheticCount++;
          if (!synthetic.isCompleted) synthetic.complete();
        }
        objcSendVoid1(application, objcSelector('sendEvent:'), event.address);
        return true;
      });
      if (!more) break;
      count++;
    }
    if (active && count == 32) Timer.run(dispatch);
  }

  void dispose() {
    active = false;
    EventLoop.clearNativeEventDispatcher();
    if (distantPast != nullptr) objcRelease(distantPast);
    if (application != nullptr) objcRelease(application);
    distantPast = nullptr;
    application = nullptr;
  }
}

Future<void> main() async {
  if (!Platform.isMacOS ||
      !EventLoop.supportsNativePump ||
      !EventLoop.supportsNativeEventDispatcher) {
    print(
        'embedded integration unsupported: native pump/dispatcher unavailable');
    return;
  }
  final mainThread = DynamicLibrary.process()
      .lookupFunction<Int32 Function(), int Function()>('pthread_main_np')();
  require(mainThread == 1, 'AppKit requires the process main thread');
  // Keep the dispatcher strongly referenced outside its native callback.
  final host = AppKitDispatcher();
  final backend = MacosEmbeddedWindowingBackend(
      runtimeStatus: () => MacosEmbeddedRuntimeStatus.active,
      requestRunApplication: host.startApplication,
      applicationName: 'DartForge integration probe');
  final message = Completer<void>();
  final timer = Completer<void>();
  final port = RawReceivePort((Object? value) {
    require(value == 23, 'isolate message value');
    if (!message.isCompleted) message.complete();
  });
  Timer? deadline;
  var initialized = false;
  EventLoop.enableNativePump();
  try {
    final injected = StateError('injected AppKit launch failure');
    final failed = MacosEmbeddedWindowingBackend(
        runtimeStatus: () => MacosEmbeddedRuntimeStatus.active,
        requestRunApplication: () {
          throw injected;
        },
        applicationName: 'DartForge failed launch probe');
    var preserved = false;
    try {
      await failed.initialize();
    } on StateError catch (error) {
      preserved = identical(error, injected);
    }
    require(preserved, 'initialization must preserve the original error');
    require(MacosEmbeddedClasses.sink == null, 'failed launch callback sink');
    ObjCAutoreleasePool.run(() {
      final application = objcSendPointer(
          objcClass('NSApplication'), objcSelector('sharedApplication'));
      require(objcSendPointer(application, objcSelector('delegate')) == nullptr,
          'failed launch application delegate');
    });
    await failed.shutdown();
    print('failed launch preserved error and cleared delegate/sink');
    await backend.initialize();
    initialized = true;
    require(backend.launchDuration != null, 'application launch notification');
    host.postSyntheticEvent();
    deadline = Timer(const Duration(milliseconds: 10), () => timer.complete());
    port.sendPort.send(23);
    await Future.wait<void>(
            [host.synthetic.future, message.future, timer.future])
        .timeout(const Duration(seconds: 5));
    require(host.syntheticCount == 1, 'exactly one synthetic event');
    print('main-thread=true launched=true synthetic=1 message=23 timer=1');
  } finally {
    deadline?.cancel();
    port.close();
    if (initialized) await backend.shutdown();
    host.dispose();
    EventLoop.disableNativePump();
  }
  print('embedded integration shutdown verified; no window or modal loop');
}

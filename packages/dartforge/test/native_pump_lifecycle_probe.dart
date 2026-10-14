import 'dart:ffi';
import 'package:dartforge/dartforge.dart';

typedef Callback = Void Function(Pointer<Void>);

void finished(Pointer<Void> _) {
  EventLoop.disableNativePump();
  print('native pump idle keepalive and callback shutdown validated');
}

void main() {
  if (!EventLoop.supportsNativePump) {
    print('native pump lifecycle fallback validated');
    return;
  }
  // Repeated installation and removal must release owner-thread resources.
  for (var round = 0; round < 3; round++) {
    EventLoop.enableNativePump();
    EventLoop.disableNativePump();
  }
  EventLoop.enableNativePump();
  final process = DynamicLibrary.process();
  final enqueue = process.lookupFunction<
      Void Function(
          Pointer<Void>, Pointer<Void>, Pointer<NativeFunction<Callback>>),
      void Function(
          Pointer<Void>, Pointer<Void>, Pointer<NativeFunction<Callback>>)>(
    'dispatch_async_f',
  );
  enqueue(process.lookup<Void>('_dispatch_main_q'), nullptr,
      Pointer.fromFunction<Callback>(finished));
  // No Dart timer or port keeps this synchronous main alive. The native pump
  // must wait for the queued callback and defer destruction until CF returns.
}

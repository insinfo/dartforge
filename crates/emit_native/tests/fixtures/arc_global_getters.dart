import 'dart:ffi';
import 'dart:io';

class UnusedHolder {
  static final DynamicLibrary unusedLibrary = Platform.isWindows
      ? DynamicLibrary.open('kernel32.dll')
      : DynamicLibrary.process();
  static final Object unusedObject = Object();
  static final int unusedScalar = 7;
}

class LiveHolder {
  static final DynamicLibrary liveLibrary = Platform.isWindows
      ? DynamicLibrary.open('kernel32.dll')
      : DynamicLibrary.process();
  static final Object liveObject = Object();
}

void main() {
  final first = LiveHolder.liveLibrary;
  final second = LiveHolder.liveLibrary;
  final symbol = Platform.isWindows ? 'GetCurrentProcessId' : 'malloc';
  print(first.providesSymbol(symbol));
  print(identical(first, second));
  print(first.handle.address == second.handle.address);
  print(identical(LiveHolder.liveObject, LiveHolder.liveObject));
  print('static library getters validated');
}

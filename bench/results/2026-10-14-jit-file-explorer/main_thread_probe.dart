import 'dart:ffi';

void main() {
  final mainThread = DynamicLibrary.open('/usr/lib/libSystem.B.dylib')
      .lookupFunction<Int32 Function(), int Function()>('pthread_main_np');
  print('pthread_main_np=${mainThread()}');
}

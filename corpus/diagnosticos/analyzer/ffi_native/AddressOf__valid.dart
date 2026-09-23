import 'dart:ffi';

@Native<Void Function()>()
external void foo();

@Native()
external void foo2();

@Native()
external Pointer<IntPtr> global;

void main() {
  print(Native.addressOf<NativeFunction<Void Function()>>(foo));
  print(Native.addressOf<NativeFunction<Void Function()>>(foo2));
  print(Native.addressOf<Pointer<IntPtr>>(global));
}

import 'dart:ffi';

@Native<Void Function()>()
external void foo();
@Native<Void Function()>()
external void bar();

void entry(bool condition) {
  print(Native.addressOf(condition ? foo : bar));
//                       ^^^^^^^^^^^^^^^^^^^^^
// [diag.argumentMustBeNative] Argument to 'Native.addressOf' must be annotated with @Native
}

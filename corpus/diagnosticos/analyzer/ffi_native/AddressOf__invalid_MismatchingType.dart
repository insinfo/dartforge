import 'dart:ffi';

@Native<Void Function()>()
external void foo();

void main() {
  print(Native.addressOf<NativeFunction<Int8 Function()>>(foo));
//      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.mustBeASubtype] The type 'Void Function()' must be a subtype of 'Int8 Function()' for 'Native.addressOf'.
}

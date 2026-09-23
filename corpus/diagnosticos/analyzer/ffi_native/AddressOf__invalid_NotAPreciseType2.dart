import 'dart:ffi';

@Native()
external void foo();

void main() => print(Native.addressOf<NativeFunction>(foo));
//                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.mustBeASubtype] The type 'Void Function()' must be a subtype of 'Function' for 'Native.addressOf'.

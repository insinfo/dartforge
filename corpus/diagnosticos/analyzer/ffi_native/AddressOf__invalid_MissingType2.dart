import 'dart:ffi';

@Native()
external void foo();

void main() {
  print(Native.addressOf(foo));
//      ^^^^^^^^^^^^^^^^^^^^^
// [diag.mustBeANativeFunctionType] The type 'NativeType' given to 'Native.addressOf' must be a valid 'dart:ffi' native function type.
}

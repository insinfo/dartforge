import 'dart:ffi';

@Native()
external Pointer<IntPtr> global;

void main() => print(Native.addressOf(global));
//                   ^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.mustBeASubtype] The type 'Pointer<IntPtr>' must be a subtype of 'NativeType' for 'Native.addressOf'.

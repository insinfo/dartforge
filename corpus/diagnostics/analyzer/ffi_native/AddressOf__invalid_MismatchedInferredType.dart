import 'dart:ffi';

@Native()
external Pointer<IntPtr> global;

void main() => print(Native.addressOf<Pointer<Double>>(global));
//                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.mustBeASubtype] The type 'Pointer<IntPtr>' must be a subtype of 'Pointer<Double>' for 'Native.addressOf'.

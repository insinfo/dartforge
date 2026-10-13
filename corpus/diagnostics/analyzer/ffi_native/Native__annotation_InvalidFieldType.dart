import 'dart:ffi';

@Native<IntPtr>()
external int foo();
//           ^^^
// [diag.mustBeANativeFunctionType] The type 'IntPtr' given to 'Native' must be a valid 'dart:ffi' native function type.

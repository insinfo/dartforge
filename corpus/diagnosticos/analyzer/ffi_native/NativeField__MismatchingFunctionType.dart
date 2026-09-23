import 'dart:ffi';

@Native<NativeFunction<Double Function()>>()
external int Function() field;
//                      ^^^^^
// [diag.mustBeASubtype] The type 'int Function()' must be a subtype of 'NativeFunction<Double Function()>' for 'Native'.

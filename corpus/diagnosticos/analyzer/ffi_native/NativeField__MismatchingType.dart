import 'dart:ffi';

@Native<Double>()
external int field;
//           ^^^^^
// [diag.mustBeASubtype] The type 'int' must be a subtype of 'Double' for 'Native'.

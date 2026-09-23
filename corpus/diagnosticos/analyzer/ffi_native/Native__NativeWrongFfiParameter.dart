import 'dart:ffi';
@Native<IntPtr Function(Double)>()
external int wrongFfiParameter(int v);
//           ^^^^^^^^^^^^^^^^^
// [diag.mustBeASubtype] The type 'IntPtr Function(Double)' must be a subtype of 'int Function(int)' for 'Native'.

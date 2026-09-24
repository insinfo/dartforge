import 'dart:ffi';
@Native<IntPtr Function(Double)>(symbol: 'doesntmatter')
external int wrongFfiParameter(int v);
//           ^^^^^^^^^^^^^^^^^
// [diag.mustBeASubtype] The type 'IntPtr Function(Double)' must be a subtype of 'int Function(int)' for 'Native'.

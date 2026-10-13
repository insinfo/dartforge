import 'dart:ffi';
@Native<IntPtr Function(int)>(symbol: 'doesntmatter')
external int nonFfiParameter(int v);
//           ^^^^^^^^^^^^^^^
// [diag.mustBeANativeFunctionType] The type 'IntPtr Function(int)' given to 'Native' must be a valid 'dart:ffi' native function type.

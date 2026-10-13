import 'dart:ffi';
@Native<double Function(IntPtr)>(symbol: 'doesntmatter')
external double nonFfiReturnType(int v);
//              ^^^^^^^^^^^^^^^^
// [diag.mustBeANativeFunctionType] The type 'double Function(IntPtr)' given to 'Native' must be a valid 'dart:ffi' native function type.

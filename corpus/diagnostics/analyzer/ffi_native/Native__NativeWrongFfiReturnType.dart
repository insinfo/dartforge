import 'dart:ffi';
@Native<IntPtr Function(IntPtr)>()
external double wrongFfiReturnType(int v);
//              ^^^^^^^^^^^^^^^^^^
// [diag.mustBeASubtype] The type 'IntPtr Function(IntPtr)' must be a subtype of 'double Function(int)' for 'Native'.

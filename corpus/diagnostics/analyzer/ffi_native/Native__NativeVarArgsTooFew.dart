import 'dart:ffi';
@Native<Int8 Function(Int64, VarArgs<(Int32, Double)>)>()
external int doesntMatter(int x, int y);
//           ^^^^^^^^^^^^
// [diag.ffiNativeUnexpectedNumberOfParameters] Unexpected number of Native annotation parameters. Expected 3 but has 2.

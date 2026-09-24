import 'dart:ffi';
@Native<Void Function(Double)>()
external void doesntMatter(double x, double y);
//            ^^^^^^^^^^^^
// [diag.ffiNativeUnexpectedNumberOfParameters] Unexpected number of Native annotation parameters. Expected 1 but has 2.

import 'dart:ffi';
@Native<Void Function(Double, Double)>(symbol: 'DoesntMatter')
external void doesntMatter(double x);
//            ^^^^^^^^^^^^
// [diag.ffiNativeUnexpectedNumberOfParameters] Unexpected number of Native annotation parameters. Expected 2 but has 1.

import 'dart:ffi';
class K {
  @Native<Void Function(Double)>(symbol: 'DoesntMatter')
  external void doesntMatter(double x);
//              ^^^^^^^^^^^^
// [diag.ffiNativeUnexpectedNumberOfParametersWithReceiver] Unexpected number of Native annotation parameters. Expected 2 but has 1. Native instance method annotation must have receiver as first argument.
}

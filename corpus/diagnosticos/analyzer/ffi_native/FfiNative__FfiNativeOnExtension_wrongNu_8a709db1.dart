import 'dart:ffi';

extension on int {
  @Native<Bool Function(Int64)>(symbol: 'x')
  external bool f(int m);
//              ^
// [diag.ffiNativeUnexpectedNumberOfParameters] Unexpected number of Native annotation parameters. Expected 2 but has 1.
}

void g() {
  0.f(0);
}

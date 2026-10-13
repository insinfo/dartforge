import 'dart:ffi';

extension on double {
  @Native<Bool Function(Int64, Int64)>(symbol: 'Dart_PostInteger')
  external bool postInteger(int message);
//              ^^^^^^^^^^^
// [diag.mustBeASubtype] The type 'Bool Function(Int64, Int64)' must be a subtype of 'bool Function(double, int)' for 'Native'.
}

void f() {
  0.0.postInteger(0);
}

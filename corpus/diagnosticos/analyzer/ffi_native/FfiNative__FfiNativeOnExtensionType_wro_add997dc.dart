import 'dart:ffi';

extension type NativeSendPort(int id) {
  @Native<Bool Function(Int64, Int64)>(symbol: 'Dart_PostInteger')
  external bool postInteger(int message);
//              ^^^^^^^^^^^
// [diag.mustBeASubtype] The type 'Bool Function(Int64, Int64)' must be a subtype of 'bool Function(NativeSendPort, int)' for 'Native'.
}

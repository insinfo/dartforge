import 'dart:ffi';

extension type InvalidNativeSendPort._(double id) {
  @Native<Bool Function(Int64, Int64)>(symbol: 'Dart_PostInteger')
  external bool postInteger(int message);
//              ^^^^^^^^^^^
// [diag.mustBeASubtype] The type 'Bool Function(Int64, Int64)' must be a subtype of 'bool Function(InvalidNativeSendPort, int)' for 'Native'.
}

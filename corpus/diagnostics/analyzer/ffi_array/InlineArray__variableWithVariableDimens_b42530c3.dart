import 'dart:ffi';

final class MyStruct extends Struct {
  @Array.variableWithVariableDimension(-1)
//                                     ^^
// [diag.negativeVariableDimension] The variable dimension of a variable-length array must be non-negative.
  external Array<Int8> arr;
}

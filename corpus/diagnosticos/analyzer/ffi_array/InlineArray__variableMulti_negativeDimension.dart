import 'dart:ffi';

final class MyStruct extends Struct {
  @Array.variableMulti(variableDimension: -1, [2, 2])
//                                        ^^
// [diag.negativeVariableDimension] The variable dimension of a variable-length array must be non-negative.
  external Array<Array<Array<Int8>>> arr;
}

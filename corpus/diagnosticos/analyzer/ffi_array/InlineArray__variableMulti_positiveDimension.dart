import 'dart:ffi';

final class MyStruct extends Struct {
  @Array.variableMulti(variableDimension: 1, [2, 2])
  external Array<Array<Array<Int8>>> arr;
}

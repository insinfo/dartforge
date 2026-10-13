import 'dart:ffi';

final class MyStruct extends Struct {
  @Array.variableWithVariableDimension(0)
  external Array<Int8> arr;
}

import 'dart:ffi';

final class MyStruct extends Struct {
  @Array.variable(1)
  external Array<Array<Int8>> arr;
}

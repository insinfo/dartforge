import 'dart:ffi';

final class MyStruct extends Struct {
  @Array.multi([2, 2])
  external Array<Array<Int8>> arr;
}

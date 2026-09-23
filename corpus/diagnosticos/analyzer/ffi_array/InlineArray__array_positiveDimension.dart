import 'dart:ffi';

final class MyStruct extends Struct {
  @Array(1)
  external Array<Int8> arr;
}

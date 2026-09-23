import 'dart:ffi';

final class MyStruct extends Struct {
  @Array.variable()
  external Array<Int8> arr;
}

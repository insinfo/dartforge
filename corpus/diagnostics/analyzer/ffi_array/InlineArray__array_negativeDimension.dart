import 'dart:ffi';

final class MyStruct extends Struct {
  @Array(-1)
//       ^^
// [diag.nonPositiveArrayDimension] Array dimensions must be positive numbers.
  external Array<Int8> arr;
}

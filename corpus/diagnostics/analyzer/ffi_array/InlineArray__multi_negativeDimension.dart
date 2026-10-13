import 'dart:ffi';

final class MyStruct extends Struct {
  @Array.multi([-2, 2])
//              ^^
// [diag.nonPositiveArrayDimension] Array dimensions must be positive numbers.
  external Array<Array<Int8>> arr;
}

import "dart:ffi";

final class MyStruct extends Struct {
  @Array.multi([1, 2, 3, -4, 5, 6])
//                       ^^
// [diag.nonPositiveArrayDimension] Array dimensions must be positive numbers.
  external Array<Array<Array<Array<Array<Array<Uint8>>>>>> a0;
}

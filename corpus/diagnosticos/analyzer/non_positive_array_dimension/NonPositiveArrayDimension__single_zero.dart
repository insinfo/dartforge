import "dart:ffi";

final class MyStruct extends Struct {
  @Array(0)
//       ^
// [diag.nonPositiveArrayDimension] Array dimensions must be positive numbers.
  external Array<Uint8> a0;
}

import "dart:ffi";

final class MyStruct extends Struct {
  @Array.multi([1])
  external Array<Uint8> a0;
}

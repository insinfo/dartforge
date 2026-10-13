import 'dart:ffi';

const EIGHT = 8;

final class Struct8BytesInlineArrayInt extends Struct {
  @Array(EIGHT)
  external Array<Uint8> a0;
}

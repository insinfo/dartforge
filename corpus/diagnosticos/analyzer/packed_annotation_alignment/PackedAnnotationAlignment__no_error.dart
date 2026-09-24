import 'dart:ffi';

@Packed(1)
final class C extends Struct {
  external Pointer<Uint8> notEmpty;
}

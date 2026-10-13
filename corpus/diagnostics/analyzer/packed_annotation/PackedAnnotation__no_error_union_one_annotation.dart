import 'dart:ffi';

@Packed(1)
final class C extends Union {
  external Pointer<Uint8> notEmpty;
}

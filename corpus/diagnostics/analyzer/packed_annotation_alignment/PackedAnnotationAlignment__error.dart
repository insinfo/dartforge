import 'dart:ffi';

@Packed(3)
//      ^
// [diag.packedAnnotationAlignment] Only packing to 1, 2, 4, 8, and 16 bytes is supported.
final class C extends Struct {
  external Pointer<Uint8> notEmpty;
}

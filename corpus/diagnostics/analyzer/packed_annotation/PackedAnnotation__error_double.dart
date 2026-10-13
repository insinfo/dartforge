import 'dart:ffi';

@Packed(1)
@Packed(1)
// [diag.packedAnnotation][column 1][length 10] Structs must have at most one 'Packed' annotation.
final class C extends Struct {
  external Pointer<Uint8> notEmpty;
}

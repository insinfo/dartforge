import 'dart:ffi';

@Packed()
// [diag.packedAnnotationAlignment][column 1][length 9] Only packing to 1, 2, 4, 8, and 16 bytes is supported.
//      ^
// [diag.notEnoughPositionalArgumentsNameSingular] 1 positional argument expected by 'Packed.new', but 0 found.
final class C extends Struct {
  external Pointer<Uint8> notEmpty;
}

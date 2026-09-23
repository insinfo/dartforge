import 'dart:ffi';
final class C extends Struct {
  @Int32()
//^^^^^^^^
// [diag.annotationOnPointerField] Fields in a struct class whose type is 'Pointer' shouldn't have any annotations.
  external Pointer<Float> x;
}

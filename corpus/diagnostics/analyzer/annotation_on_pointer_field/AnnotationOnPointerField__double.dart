import 'dart:ffi';
final class C extends Struct {
  @Double()
//^^^^^^^^^
// [diag.annotationOnPointerField] Fields in a struct class whose type is 'Pointer' shouldn't have any annotations.
  external Pointer<Int8> x;
}

import 'dart:ffi';

final class C extends Struct {
  @Array(8)
  @Array(8)
//^^^^^^^^^
// [diag.extraSizeAnnotationCarray] 'Array's must have exactly one 'Array' annotation.
  external Array<Uint8> a0;
}

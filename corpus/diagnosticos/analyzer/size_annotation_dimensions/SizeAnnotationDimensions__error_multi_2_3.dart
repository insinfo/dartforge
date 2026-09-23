import 'dart:ffi';

final class C extends Struct {
  @Array.multi([8, 8])
//^^^^^^^^^^^^^^^^^^^^
// [diag.sizeAnnotationDimensions] 'Array's must have an 'Array' annotation that matches the dimensions.
  external Array<Array<Array<Uint8>>> a0;
}

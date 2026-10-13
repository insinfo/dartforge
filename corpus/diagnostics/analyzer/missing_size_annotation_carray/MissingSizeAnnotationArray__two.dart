import 'dart:ffi';

final class C extends Struct {
  external Array<Uint8> a0;
//         ^^^^^^^^^^^^
// [diag.missingSizeAnnotationCarray] Fields of type 'Array' must have exactly one 'Array' annotation.
}

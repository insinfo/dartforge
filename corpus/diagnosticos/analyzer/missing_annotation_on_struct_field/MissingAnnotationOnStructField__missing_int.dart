import 'dart:ffi';
final class C extends Struct {
  external int x;
//         ^^^
// [diag.missingAnnotationOnStructField] Fields of type 'int' in a subclass of 'Struct' must have an annotation indicating the native type.
}

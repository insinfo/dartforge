import 'dart:ffi';
final class C extends Struct {
  @Double()
//^^^^^^^^^
// [diag.mismatchedAnnotationOnStructField] The annotation doesn't match the declared type of the field.
  external int x;
}

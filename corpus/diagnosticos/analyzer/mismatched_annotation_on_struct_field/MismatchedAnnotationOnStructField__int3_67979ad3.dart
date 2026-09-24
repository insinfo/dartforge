import 'dart:ffi';
final class C extends Struct {
  @Int32()
//^^^^^^^^
// [diag.mismatchedAnnotationOnStructField] The annotation doesn't match the declared type of the field.
  external double x;
}

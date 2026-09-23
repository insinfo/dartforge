import 'dart:ffi';
final class C extends Struct {
  @Int32()
  @Int16()
//^^^^^^^^
// [diag.extraAnnotationOnStructField] Fields in a struct class must have exactly one annotation indicating the native type.
  external int x;
}

class A {
  final int? n1;
  A({n1});
  augment A({this.n1});
//           ^^^^^^^
// [diag.fieldInitializingFormalNotAssignable] The parameter type 'dynamic' is incompatible with the field type 'int?'.
}

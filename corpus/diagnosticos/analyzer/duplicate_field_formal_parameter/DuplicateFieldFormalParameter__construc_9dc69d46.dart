class A {
  final x;
  A(this.x, this.x) {}
//       ^
// [context 1] The first definition of this name.
//               ^
// [diag.duplicateFieldFormalParameter][context 1] The field 'x' can't be initialized by multiple parameters in the same constructor.
}

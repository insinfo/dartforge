class A(this.f, this.f) {
//           ^
// [context 1] The first definition of this name.
//                   ^
// [diag.duplicateFieldFormalParameter][context 1] The field 'f' can't be initialized by multiple parameters in the same constructor.
  int f;
}

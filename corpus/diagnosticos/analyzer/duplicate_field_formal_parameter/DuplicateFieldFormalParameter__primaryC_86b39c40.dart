class A([this.a = 0, this.a = 1]) {
//            ^
// [context 1] The first definition of this name.
//                        ^
// [diag.duplicateFieldFormalParameter][context 1] The field 'a' can't be initialized by multiple parameters in the same constructor.
  int a;
}

class A(int a, this.a) {
//          ^
// [context 1] The first definition of this name.
//                  ^
// [diag.duplicateDefinition][context 1] The name 'a' is already defined.
  int a;
}

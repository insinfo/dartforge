enum E({required final int foo}) {
//                         ^^^
// [context 1] The first definition of this name.
  v(foo: 0);
  void foo() {}
//     ^^^
// [diag.duplicateDefinition][context 1] The name 'foo' is already defined.
}

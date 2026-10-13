class A {}
extension E on A {
  void foo() {}
//     ^^^
// [context 1] The first definition of this name.
  void foo() {}
//     ^^^
// [diag.duplicateDefinition][context 1] The name 'foo' is already defined.
}

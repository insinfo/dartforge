extension E on int {
  void foo() {}
//     ^^^
// [context 1] The first definition of this name.
}

augment extension E {
  void foo() {}
//     ^^^
// [diag.duplicateDefinition][context 1] The name 'foo' is already defined.
}

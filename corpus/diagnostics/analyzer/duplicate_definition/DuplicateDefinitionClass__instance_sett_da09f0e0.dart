class A {
  set foo(_) {}
//    ^^^
// [context 1] The first definition of this name.
}

augment class A {
  void foo() {}
//     ^^^
// [diag.duplicateDefinition][context 1] The name 'foo' is already defined.
}

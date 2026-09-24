class A {
  static void foo() {}
//            ^^^
// [context 1] The first definition of this name.
}

augment class A {
  static void foo() {}
//            ^^^
// [diag.duplicateDefinition][context 1] The name 'foo' is already defined.
}

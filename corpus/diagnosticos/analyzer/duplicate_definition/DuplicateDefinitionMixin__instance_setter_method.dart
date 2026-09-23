mixin M {
  set foo(_) {}
//    ^^^
// [context 1] The first definition of this name.
  void foo() {}
//     ^^^
// [diag.duplicateDefinition][context 1] The name 'foo' is already defined.
}

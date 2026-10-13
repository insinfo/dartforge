enum E {
  v;
  void foo() {}
//     ^^^
// [context 1] The first definition of this name.
  set foo(_) {}
//    ^^^
// [diag.duplicateDefinition][context 1] The name 'foo' is already defined.
}

extension type E(int it) {
  void set foo(_) {}
//         ^^^
// [context 1] The first definition of this name.
  void set foo(_) {}
//         ^^^
// [diag.duplicateDefinition][context 1] The name 'foo' is already defined.
}

extension type E(int it) {
//                   ^^
// [context 1] The first definition of this name.
  void it() {}
//     ^^
// [diag.duplicateDefinition][context 1] The name 'it' is already defined.
}

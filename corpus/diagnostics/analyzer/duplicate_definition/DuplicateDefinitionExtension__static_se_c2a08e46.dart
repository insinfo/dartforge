extension E on int {
  static void set foo(_) {}
//                ^^^
// [context 1] The first definition of this name.
}

augment extension E {
  static void set foo(_) {}
//                ^^^
// [diag.duplicateDefinition][context 1] The name 'foo' is already defined.
}

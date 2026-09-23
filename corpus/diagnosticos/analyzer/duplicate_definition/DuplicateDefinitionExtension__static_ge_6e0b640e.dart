extension E on int {
  static int get foo => 0;
//               ^^^
// [context 1] The first definition of this name.
}

augment extension E {
  static int get foo => 0;
//               ^^^
// [diag.duplicateDefinition][context 1] The name 'foo' is already defined.
}

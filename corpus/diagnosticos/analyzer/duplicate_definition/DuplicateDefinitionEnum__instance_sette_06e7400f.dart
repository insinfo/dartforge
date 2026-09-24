enum E {
  v;
  void set foo(_) {}
//         ^^^
// [context 1] The first definition of this name.
}

augment enum E {;
  void set foo(_) {}
//         ^^^
// [diag.duplicateDefinition][context 1] The name 'foo' is already defined.
}

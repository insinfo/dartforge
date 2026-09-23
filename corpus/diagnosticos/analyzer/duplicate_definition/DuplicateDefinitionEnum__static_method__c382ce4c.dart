enum E {
  v;
  static void foo() {}
//            ^^^
// [context 1] The first definition of this name.
}

augment enum E {;
  static void foo() {}
//            ^^^
// [diag.duplicateDefinition][context 1] The name 'foo' is already defined.
}

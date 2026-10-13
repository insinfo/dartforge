enum E {
  v;
  int get foo => 0;
//        ^^^
// [context 1] The first definition of this name.
}

augment enum E {;
  int get foo => 0;
//        ^^^
// [diag.duplicateDefinition][context 1] The name 'foo' is already defined.
}

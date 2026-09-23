enum E {
  foo;
//^^^
// [context 1] The first definition of this name.
  static int foo = 0;
//           ^^^
// [diag.duplicateDefinition][context 1] The name 'foo' is already defined.
}

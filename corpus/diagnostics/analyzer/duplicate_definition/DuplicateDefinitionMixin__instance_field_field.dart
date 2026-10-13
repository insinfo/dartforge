mixin M {
  int foo = 0;
//    ^^^
// [context 1] The first definition of this name.
  int foo = 0;
//    ^^^
// [diag.duplicateDefinition][context 1] The name 'foo' is already defined.
}

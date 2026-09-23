enum E {
  foo, foo
//^^^
// [context 1] The first definition of this name.
//     ^^^
// [diag.duplicateDefinition][context 1] The name 'foo' is already defined.
}

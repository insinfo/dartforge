class C(var int foo) {
//              ^^^
// [context 1] The first definition of this name.
  set foo(int x) {}
//    ^^^
// [diag.duplicateDefinition][context 1] The name 'foo' is already defined.
}

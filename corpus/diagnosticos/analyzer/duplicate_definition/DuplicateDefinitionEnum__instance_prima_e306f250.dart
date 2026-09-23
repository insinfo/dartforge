enum E(var int foo) {
//             ^^^
// [context 1] The first definition of this name.
// [diag.nonFinalFieldInEnum] Enums can only declare final fields.
  v(0);
  set foo(int x) {}
//    ^^^
// [diag.duplicateDefinition][context 1] The name 'foo' is already defined.
}

class A {}
extension E on A {
  static int foo = 0;
//           ^^^
// [context 1] The first definition of this name.
  static int get foo => 0;
//               ^^^
// [diag.duplicateDefinition][context 1] The name 'foo' is already defined.
}

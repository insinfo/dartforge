class A {}
extension E on A {
  static void foo() {}
//            ^^^
// [context 1] The first definition of this name.
  static set foo(_) {}
//           ^^^
// [diag.duplicateDefinition][context 1] The name 'foo' is already defined.
}

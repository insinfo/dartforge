mixin M {
  static set foo(_) {}
//           ^^^
// [context 1] The first definition of this name.
  static void foo() {}
//            ^^^
// [diag.duplicateDefinition][context 1] The name 'foo' is already defined.
}

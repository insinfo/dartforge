class C {
  static late final int foo;
//                      ^^^
// [context 1] The first definition of this name.
  static set foo(int x) {}
//           ^^^
// [diag.duplicateDefinition][context 1] The name 'foo' is already defined.
}

class A {
  A();
  final int v;
//          ^
// [context 1] The first definition of this name.
  final int v;
//          ^
// [diag.duplicateDefinition][context 1] The name 'v' is already defined.
}

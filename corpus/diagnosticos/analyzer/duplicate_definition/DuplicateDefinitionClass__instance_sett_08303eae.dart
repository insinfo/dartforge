class A {
  void set b(int? value) {}
  int? get b => null;
//         ^
// [context 1] The first definition of this name.
  int? get b => 0;
//         ^
// [diag.duplicateDefinition][context 1] The name 'b' is already defined.
}

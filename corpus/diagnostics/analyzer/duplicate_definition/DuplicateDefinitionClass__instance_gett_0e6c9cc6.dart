class A {
  int? get b => null;
//         ^
// [context 1] The first definition of this name.
  void set b(int? value) {}
  int? get b => 0;
//         ^
// [diag.duplicateDefinition][context 1] The name 'b' is already defined.
}

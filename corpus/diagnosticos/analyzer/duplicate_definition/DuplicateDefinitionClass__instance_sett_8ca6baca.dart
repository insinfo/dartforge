class A {
  void set b(int? value) {}
//         ^
// [context 1] The first definition of this name.
  void set b(int? value) {}
//         ^
// [diag.duplicateDefinition][context 1] The name 'b' is already defined.
  int? get b => null;
}

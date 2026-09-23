mixin M on Enum {
  void values() {}
//     ^^^^^^
// [diag.illegalEnumValuesDeclaration] An instance member named 'values' can't be declared in a class that implements 'Enum'.
}

enum E {
  v;
  void values() {}
//     ^^^^^^
// [diag.valuesDeclarationInEnum] A member named 'values' can't be declared in an enum.
}

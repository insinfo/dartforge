enum E {
  v;
  final values = [];
//      ^^^^^^
// [diag.valuesDeclarationInEnum] A member named 'values' can't be declared in an enum.
  const E();
}

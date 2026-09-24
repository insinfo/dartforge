enum E {
  v;
  static int get values => 0;
//               ^^^^^^
// [diag.valuesDeclarationInEnum] A member named 'values' can't be declared in an enum.
}

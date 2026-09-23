enum E {
  v;
  set values(_) {}
//    ^^^^^^
// [diag.valuesDeclarationInEnum] A member named 'values' can't be declared in an enum.
}

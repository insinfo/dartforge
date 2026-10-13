mixin M on Enum {
  int values = 0;
//    ^^^^^^
// [diag.illegalEnumValuesDeclaration] An instance member named 'values' can't be declared in a class that implements 'Enum'.
}

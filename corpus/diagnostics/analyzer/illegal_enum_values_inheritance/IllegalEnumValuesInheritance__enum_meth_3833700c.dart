mixin M {
  int values() => 0;
}

enum E with M {
//   ^
// [diag.illegalEnumValuesInheritance] An instance member named 'values' can't be inherited from 'M' in a class that implements 'Enum'.
  v
}

class A {
  set values(int _) {}
}

enum E implements A {
//   ^
// [diag.illegalEnumValuesInheritance] An instance member named 'values' can't be inherited from 'A' in a class that implements 'Enum'.
  v
}

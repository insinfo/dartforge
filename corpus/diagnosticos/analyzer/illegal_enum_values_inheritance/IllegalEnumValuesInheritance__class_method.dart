class A {
  void values() {}
}

abstract class B extends A implements Enum {}
//             ^
// [diag.illegalEnumValuesInheritance] An instance member named 'values' can't be inherited from 'A' in a class that implements 'Enum'.

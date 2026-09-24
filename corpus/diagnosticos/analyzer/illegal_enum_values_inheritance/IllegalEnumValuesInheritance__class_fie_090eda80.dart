mixin M {
  int values = 0;
}

abstract class B with M implements Enum {}
//             ^
// [diag.illegalEnumValuesInheritance] An instance member named 'values' can't be inherited from 'M' in a class that implements 'Enum'.

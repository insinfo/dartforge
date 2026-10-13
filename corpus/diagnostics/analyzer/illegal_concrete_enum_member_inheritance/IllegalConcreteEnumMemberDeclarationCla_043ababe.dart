mixin M {
  int hashCode = 0;
}

abstract class B with M implements Enum {}
//             ^
// [diag.illegalConcreteEnumMemberInheritance] A concrete instance member named 'hashCode' can't be inherited from 'M' in a class that implements 'Enum'.

class A {
  int hashCode = 0;
}

abstract class B extends A implements Enum {}
//             ^
// [diag.illegalConcreteEnumMemberInheritance] A concrete instance member named 'hashCode' can't be inherited from 'A' in a class that implements 'Enum'.

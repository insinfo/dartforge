class A {
  bool operator ==(Object other) => false;
}

abstract class B extends A implements Enum {}
//             ^
// [diag.illegalConcreteEnumMemberInheritance] A concrete instance member named '==' can't be inherited from 'A' in a class that implements 'Enum'.

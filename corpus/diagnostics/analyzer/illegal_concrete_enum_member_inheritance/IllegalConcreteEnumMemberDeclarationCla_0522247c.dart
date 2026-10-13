mixin M {
  bool operator ==(Object other) => false;
}

abstract class B with M implements Enum {}
//             ^
// [diag.illegalConcreteEnumMemberInheritance] A concrete instance member named '==' can't be inherited from 'M' in a class that implements 'Enum'.

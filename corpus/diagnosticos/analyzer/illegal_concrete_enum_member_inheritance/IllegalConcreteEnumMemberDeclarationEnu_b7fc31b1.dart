mixin M {
  bool operator ==(Object other) => false;
}

enum E with M {
//   ^
// [diag.illegalConcreteEnumMemberInheritance] A concrete instance member named '==' can't be inherited from 'M' in a class that implements 'Enum'.
  v;
}

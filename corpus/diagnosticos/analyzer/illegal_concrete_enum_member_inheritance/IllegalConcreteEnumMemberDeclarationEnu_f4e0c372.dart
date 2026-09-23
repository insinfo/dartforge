mixin M {
  int get hashCode => 0;
}

enum E with M {
//   ^
// [diag.illegalConcreteEnumMemberInheritance] A concrete instance member named 'hashCode' can't be inherited from 'M' in a class that implements 'Enum'.
  v;
}

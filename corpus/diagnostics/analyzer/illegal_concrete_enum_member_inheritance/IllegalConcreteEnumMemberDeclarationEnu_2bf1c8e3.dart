mixin M {
  int get index => 0;
}

enum E with M {
//   ^
// [diag.illegalConcreteEnumMemberInheritance] A concrete instance member named 'index' can't be inherited from 'M' in a class that implements 'Enum'.
  v;
}

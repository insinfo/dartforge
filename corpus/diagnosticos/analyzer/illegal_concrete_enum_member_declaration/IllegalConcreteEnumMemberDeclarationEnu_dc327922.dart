enum E {
  v;
  int get index => 0;
//        ^^^^^
// [diag.illegalConcreteEnumMemberDeclaration] A concrete instance member named 'index' can't be declared in a class that implements 'Enum'.
}

enum E {
  v;
  final int index;
//          ^^^^^
// [diag.illegalConcreteEnumMemberDeclaration] A concrete instance member named 'index' can't be declared in a class that implements 'Enum'.
  const E();
}

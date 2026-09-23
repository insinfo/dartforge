enum E {
  v;
  bool operator ==(Object other) => false;
//              ^^
// [diag.illegalConcreteEnumMemberDeclaration] A concrete instance member named '==' can't be declared in a class that implements 'Enum'.
}

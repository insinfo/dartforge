class MyClass {
  static void sm() {}
}
extension MyExtension on MyClass {
  void m() {
    sm();
//  ^^
// [diag.unqualifiedReferenceToStaticMemberOfExtendedType] Static members from the extended type or one of its superclasses must be qualified by the name of the defining type.
  }
}

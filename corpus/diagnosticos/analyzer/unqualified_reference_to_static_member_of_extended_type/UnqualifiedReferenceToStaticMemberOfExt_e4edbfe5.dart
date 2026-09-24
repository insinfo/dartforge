class MyClass {
  static int get zero => 0;
}
extension MyExtension on MyClass {
  void m() {
    zero;
//  ^^^^
// [diag.unqualifiedReferenceToStaticMemberOfExtendedType] Static members from the extended type or one of its superclasses must be qualified by the name of the defining type.
  }
}

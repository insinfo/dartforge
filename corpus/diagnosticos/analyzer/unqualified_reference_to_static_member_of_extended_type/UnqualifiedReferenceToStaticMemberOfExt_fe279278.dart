class MyClass {
  static void sm<T>() {}
}
extension MyExtension on MyClass {
  void m() {
    sm<int>;
//  ^^
// [diag.unqualifiedReferenceToStaticMemberOfExtendedType] Static members from the extended type or one of its superclasses must be qualified by the name of the defining type.
  }
}

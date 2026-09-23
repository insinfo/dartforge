class MyClass {
  static int get x => 0;
  static set x(int _) {}
}

extension MyExtension on MyClass {
  void f() {
    x = 0;
//  ^
// [diag.unqualifiedReferenceToStaticMemberOfExtendedType] Static members from the extended type or one of its superclasses must be qualified by the name of the defining type.
    x += 1;
//  ^
// [diag.unqualifiedReferenceToStaticMemberOfExtendedType] Static members from the extended type or one of its superclasses must be qualified by the name of the defining type.
    ++x;
//    ^
// [diag.unqualifiedReferenceToStaticMemberOfExtendedType] Static members from the extended type or one of its superclasses must be qualified by the name of the defining type.
    x++;
//  ^
// [diag.unqualifiedReferenceToStaticMemberOfExtendedType] Static members from the extended type or one of its superclasses must be qualified by the name of the defining type.
  }
}

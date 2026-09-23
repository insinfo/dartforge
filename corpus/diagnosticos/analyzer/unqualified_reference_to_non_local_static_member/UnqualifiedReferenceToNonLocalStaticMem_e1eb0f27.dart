class A {
  static int get x => 0;
  static set x(int _) {}
}
class B extends A {
  void f() {
    x = 0;
//  ^
// [diag.unqualifiedReferenceToNonLocalStaticMember] Static members from supertypes must be qualified by the name of the defining type.
    x += 1;
//  ^
// [diag.unqualifiedReferenceToNonLocalStaticMember] Static members from supertypes must be qualified by the name of the defining type.
    ++x;
//    ^
// [diag.unqualifiedReferenceToNonLocalStaticMember] Static members from supertypes must be qualified by the name of the defining type.
    x++;
//  ^
// [diag.unqualifiedReferenceToNonLocalStaticMember] Static members from supertypes must be qualified by the name of the defining type.
  }
}

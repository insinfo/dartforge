class A {
  static int get a => 0;
}
class B extends A {
  int b() {
    return a;
//         ^
// [diag.unqualifiedReferenceToNonLocalStaticMember] Static members from supertypes must be qualified by the name of the defining type.
  }
}

abstract base class A {
  static void a() {}
}
final class B extends A {
  void b() {
    a();
//  ^
// [diag.unqualifiedReferenceToNonLocalStaticMember] Static members from supertypes must be qualified by the name of the defining type.
  }
}

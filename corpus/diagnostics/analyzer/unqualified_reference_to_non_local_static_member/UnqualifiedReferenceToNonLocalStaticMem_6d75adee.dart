class A {
  static void a<T>() {}
}
class B extends A {
  void b() {
    a<int>;
//  ^
// [diag.unqualifiedReferenceToNonLocalStaticMember] Static members from supertypes must be qualified by the name of the defining type.
  }
}

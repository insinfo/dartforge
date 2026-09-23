class A {
  static void Function() get a => () {};
}

class B extends A {
  void b() {
    a();
//  ^
// [diag.unqualifiedReferenceToNonLocalStaticMember] Static members from supertypes must be qualified by the name of the defining type.
  }
}

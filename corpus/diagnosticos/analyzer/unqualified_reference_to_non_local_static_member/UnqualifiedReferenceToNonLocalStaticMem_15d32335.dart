class A {
  static int foo = 1;
}

class B extends A {
  static bar() {
    foo.abs();
//  ^^^
// [diag.unqualifiedReferenceToNonLocalStaticMember] Static members from supertypes must be qualified by the name of the defining type.
  }
}

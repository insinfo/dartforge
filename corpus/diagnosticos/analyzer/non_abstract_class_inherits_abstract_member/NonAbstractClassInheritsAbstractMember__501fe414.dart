class A {
  external int x;
}
class B implements A {
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'getter A.x'.
  void set x(int value) {}
}

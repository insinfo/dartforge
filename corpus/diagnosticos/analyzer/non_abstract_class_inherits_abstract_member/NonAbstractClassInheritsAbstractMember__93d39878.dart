class A {
  external final int x;
}
class B implements A {}
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'getter A.x'.

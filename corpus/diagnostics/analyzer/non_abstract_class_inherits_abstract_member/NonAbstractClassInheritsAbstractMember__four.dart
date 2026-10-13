abstract class A {
  m();
  n();
  o();
  p();
}
class C extends A {
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberFour] Missing concrete implementations of 'A.m', 'A.n', 'A.o', and 'A.p'.
}

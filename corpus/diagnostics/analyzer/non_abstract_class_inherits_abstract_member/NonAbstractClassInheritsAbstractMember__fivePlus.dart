abstract class A {
  m();
  n();
  o();
  p();
  q();
}
class C extends A {
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberFivePlus] Missing concrete implementations of 'A.m', 'A.n', 'A.o', 'A.p', and 1 more.
}

abstract class A {
  m();
  n();
  o();
}
class C extends A {
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberThree] Missing concrete implementations of 'A.m', 'A.n', and 'A.o'.
}

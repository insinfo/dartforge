abstract class A {
  m();
  n();
}
class C extends A {
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberTwo] Missing concrete implementations of 'A.m' and 'A.n'.
}

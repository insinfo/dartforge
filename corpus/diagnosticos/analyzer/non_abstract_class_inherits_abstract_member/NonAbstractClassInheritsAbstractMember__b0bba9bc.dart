abstract class A {
  m(p);
}
class C extends A {
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'A.m'.
}

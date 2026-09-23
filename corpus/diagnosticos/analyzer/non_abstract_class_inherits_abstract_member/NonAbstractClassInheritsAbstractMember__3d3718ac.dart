class I {
  noSuchMethod(v) => '';
}
abstract class A {
  m();
}
class B extends A implements I {
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'A.m'.
}

class I {
  m(p) {}
  noSuchMethod(v) => null;
}
class C implements I {
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'I.m'.
}

class I {
  m(p) {}
}
class C implements I {
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'I.m'.
}

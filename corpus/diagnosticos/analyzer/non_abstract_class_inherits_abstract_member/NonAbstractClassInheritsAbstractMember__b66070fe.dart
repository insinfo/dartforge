class I {
  set s(int i) {}
}
class C implements I {
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'setter I.s'.
}

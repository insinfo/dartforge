class I {
  var v;
}
class C implements I {
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'setter I.v'.
  get v => 1;
}

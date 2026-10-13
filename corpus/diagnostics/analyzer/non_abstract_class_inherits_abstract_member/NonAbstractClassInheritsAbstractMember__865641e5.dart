class I {
  var v;
}
class C implements I {
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'getter I.v'.
  set v(_) {}
}

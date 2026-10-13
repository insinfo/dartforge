class I {
  var v;
}
class C implements I {
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberTwo] Missing concrete implementations of 'getter I.v' and 'setter I.v'.
}

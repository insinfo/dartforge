class I {
  int get g {return 1;}
}
class C implements I {
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'getter I.g'.
}

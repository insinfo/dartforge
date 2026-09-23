abstract class A {
  set s(int i);
}
class C extends A {
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'setter A.s'.
}

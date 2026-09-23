abstract class A {
  int get g;
}
class C extends A {
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'getter A.g'.
}

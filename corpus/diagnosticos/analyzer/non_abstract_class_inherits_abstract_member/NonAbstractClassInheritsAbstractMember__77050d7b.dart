abstract class A {
  abstract int x;
}
class B implements A {
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'setter A.x'.
  int get x => 0;
}

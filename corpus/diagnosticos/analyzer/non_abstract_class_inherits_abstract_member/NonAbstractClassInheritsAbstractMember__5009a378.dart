class A {
  get a => 'a';
}
abstract class B implements A {
  get b => 'b';
}
class C extends B {
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'getter A.a'.
}

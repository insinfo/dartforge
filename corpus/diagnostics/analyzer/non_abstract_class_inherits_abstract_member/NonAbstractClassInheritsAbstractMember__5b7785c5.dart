class A {
  external int x;
}
class B implements A {}
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberTwo] Missing concrete implementations of 'getter A.x' and 'setter A.x'.

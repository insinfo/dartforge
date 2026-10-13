class A {}
class B extends A {}
extension E on B {
  void m() {}
}
void f(A a) {
  E(a).m();
//  ^
// [diag.extensionOverrideArgumentNotAssignable] The type of the argument to the extension override 'A' isn't assignable to the extended type 'B'.
}

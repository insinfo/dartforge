class A {}
class B {}
extension E on A {
  void m() {}
}
void f(B b) {
  E(b).m();
//  ^
// [diag.extensionOverrideArgumentNotAssignable] The type of the argument to the extension override 'B' isn't assignable to the extended type 'A'.
}

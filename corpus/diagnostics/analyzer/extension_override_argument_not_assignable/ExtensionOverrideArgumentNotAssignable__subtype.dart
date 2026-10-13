class A {}
class B extends A {}
extension E on A {
  void m() {}
}
void f(B b) {
  E(b).m();
}

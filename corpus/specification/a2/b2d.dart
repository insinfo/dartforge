extension E on int { void m() {} int get g => 0; }
void f() {
  E(3)..m();
  E(3)..g..m();
  E(3);
  var x = E(3);
}
extension type A(int it) { A.n(this.it) : super(); A.k(int x) : it = x, super(); }
extension type B(int it) implements B {}
extension type C(int it) implements D {}
extension type D(int it) implements C {}

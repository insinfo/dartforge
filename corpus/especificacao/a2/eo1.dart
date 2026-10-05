class A {}
class B {}
extension E on A { void m() {} }
extension G<T extends num> on List<T> { void g() {} }
void f(B b, void v, dynamic d, A? n, List<String> ls) {
  E(b).m();
  E(v).m();
  E(d).m();
  E(n).m();
  E(n)?.m();
  G(ls).g();
  E(b, b).m();
}

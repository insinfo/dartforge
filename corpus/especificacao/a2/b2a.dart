extension E on int { void m() {} }
extension G<T, U> on T { void g() {} }
void f() {
  E<int>(1).m();
  G<int>(1).g();
  G<int, int, int>(1).g();
  G<int, String>(1).g();
}

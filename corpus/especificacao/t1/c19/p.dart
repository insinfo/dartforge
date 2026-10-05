part of "main.dart";
class B implements I {
  void m() {}
}
class C {
  int x = 0;
  void g() {
    this.x;
    this.g();
  }
}
void f(C c) {
  c.x;
  c.g();
}

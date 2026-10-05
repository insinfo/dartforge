class A {
  @deprecated
  int a = 0, b = 0;
  @deprecated
  A.n();
  A() : this.n();
  void m() { print(a + b); }
}
class B extends A {
  B() : super.n();
  @deprecated
  void old(int a) { old(1); }
}
enum E { @deprecated x, y }
void g(Object o) {
  print(E.x);
  if (o case A(a: var q)) { print(q); }
  if (o case A(:var b)) { print(b); }
}

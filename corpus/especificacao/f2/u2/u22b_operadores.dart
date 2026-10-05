class A {}
extension _E on A {
  A operator +(int o) => this;
  A operator -(int o) => this;
  int operator [](int i) => 0;
  void operator []=(int i, int v) {}
  A operator -() => this;
  A operator ~() => this;
  int call() => 0;
  bool operator <(int o) => true;
  A operator *(int o) => this;
  A operator /(int o) => this;
}
class B {}
extension _F on B {
  int operator [](int i) => 0;
  void operator []=(int i, int v) {}
}
class D {}
extension _G on D {
  int? operator [](int i) => 0;
  void operator []=(int i, int v) {}
}
class H {}
extension _I on H {
  int operator [](int i) => 0;
  void operator []=(int i, int v) {}
}
void f(A a, B b, D d, H h) {
  a + 1;
  a[0];
  -a;
  a();
  var c = a;
  c *= 2;
  print(c);
  b[0] += 1;
  d[0] ??= 1;
  h[0] = 1;
}

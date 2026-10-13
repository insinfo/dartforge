class A {
  int _v = 0;
}
extension _E on A {
  A operator +(int o) => this;
  A operator -(int o) => this;
  int operator [](int i) => 0;
  void operator []=(int i, int v) {}
  A operator -() => this;
  A operator ~() => this;
  int call() => 0;
  bool operator <(int o) => true;
}
extension _F on A {
  int operator [](int i) => 0;
  void operator []=(int i, int v) {}
  A operator *(int o) => this;
}
void f(A a, _F? nada) {
  a + 1;
  a[0];
  -a;
  a();
  var b = a;
  b *= 2;
  _F(a)[0] += 1;
  print(a._v);
}

class A { A(int a, int b); A.n(int a); }
void f(int a, int b) {}
void g(int a) {}
void h(void Function(int) cb, dynamic d) {
  f();
  f(1);
  f(1, );
  f(b: 2);
  g();
  g(x: 1);
  new A();
  new A(1);
  A.n();
  cb();
  (cb)();
  d();
  f(1, 2, 3);
  undefinedFn();
}
class B extends A { B() : super(1); B.k() : super.n(); }

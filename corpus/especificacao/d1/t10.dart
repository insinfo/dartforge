class A {
  final int x;
  const A(int v) : this.n(v ~/ 0);
  const A.n(this.x) : assert(x > 0);
  const factory A.f(int v) = A.n;
  const A.m(int v) : this.n(v);
}
const a = const A(1);
const b = const A.f(0);
const c = const A.m(0);

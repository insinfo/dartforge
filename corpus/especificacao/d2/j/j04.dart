enum E {
  a(0), b.n(n);
  final int x;
  const E(this.x);
  const E.n(this.x);
}
int n = 0;
class A { const A([Object? o]); A.nc(); }
@A(const A.nc())
@A([n])
@A(A.nc)
void f() {}

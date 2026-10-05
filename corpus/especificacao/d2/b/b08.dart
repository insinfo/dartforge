class A {
  final int x;
  const A(int p) : x = p + f(), assert(p is int), assert(g, 'm${p}');
  const A.b(int p) : x = (p as dynamic).foo;
  const A.c(List<int> p) : x = p.length + p.first;
  const A.d(int p) : this(h);
  const A.e(int p) : x = [p].length;
  const A.g(int p) : x = const [p].length;
}
int f() => 0;
bool g = true;
var h = 1;

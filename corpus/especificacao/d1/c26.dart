enum E {
  v1(v2), v2(v1);
  const E(Object o);
}
enum F {
  a(0), b('x');
  final int i;
  const F(Object o) : i = o as int;
}
enum G { g; const G() : assert(false); }

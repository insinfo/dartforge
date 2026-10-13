enum _E {
  one(), two();
  const _E();
  const _E.named();
}
typedef T = _E;
void f() {
  _E.one;
  _E.two;
}

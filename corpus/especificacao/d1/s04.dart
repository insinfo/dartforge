class A {
  const A(int x): assert(x > 0, '${throw ''}');
}
const a = const A(0);

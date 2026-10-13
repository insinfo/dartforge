class A<T extends Xyz> {}
class B extends A<int> {}
void f(Object o, Xyz x) {
  switch (x) { case 1: break; }
  var s = switch (x) { int _ => 1 };
  if (o case Xyz y) { y.foo; }
  x!;
  x?.foo;
  x ?? 1;
  o as Xyz;
  (o as Xyz).foo;
  final t = <Xyz>[];
  t.first.foo;
  int n = t.first;
  print([s, n]);
}

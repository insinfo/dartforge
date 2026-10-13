import 'dart:math' deferred as m;
import 'dart:async' deferred as a;
import 'dart:collection' as a;
m.Random? v1;
List<m.Random>? v2;
m.Random? f1(m.Random p, [m.Random? q]) => null;
typedef T = m.Random;
typedef F = m.Random Function(m.Random);
class C<X extends m.Random> {
  m.Random? fld;
  C(m.Random this.fld);
  m.Random get g => throw 0;
  factory C.r() = D<m.Random>;
}
class D<X> implements C<Never> {
  m.Random? fld;
  m.Random get g => throw 0;
}
void f(Object o) {
  o is m.Random;
  o as m.Random;
  try {} on m.Random catch (e) {}
  m.Random? loc;
  <m.Random>[];
  new m.Random();
  for (m.Random x in []) {}
  (m.Random, int)? rec;
  if (o case m.Random x) {}
  m.Undef? u;
  a.Future? af;
}

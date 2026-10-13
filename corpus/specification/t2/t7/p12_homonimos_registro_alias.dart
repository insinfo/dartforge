import 'apoio/a.dart' as a;

class C {}

typedef P<X> = Map<X, X>;

void f((a.C,) x, P<a.C> y, a.C? z, Map<a.C, C> w, a.C Function() k) {
  (C,) c1 = x;
  P<C> c2 = y;
  C? c3 = z;
  Map<C, a.C> c4 = w;
  C Function() c5 = k;
  print([c1, c2, c3, c4, c5]);
}

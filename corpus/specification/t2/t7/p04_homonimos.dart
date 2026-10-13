import 'apoio/a.dart' as a;

class C {}

void f(a.C x, List<a.C> y, void Function(a.C) z, a.G<C> w) {
  C c1 = x;
  List<C> c2 = y;
  void Function(C) c3 = z;
  a.G<a.C> c4 = w;
  int c5 = x;
  print([c1, c2, c3, c4, c5]);
}

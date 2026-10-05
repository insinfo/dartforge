import 'apoio/a.dart' as a;

class C {}

void f(a.C x, C y) {
  x.foo();
  y.bar;
  List<a.C> l = <C>[];
  print(l);
}

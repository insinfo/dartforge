typedef F<X> = void Function(X);

void g(int Function<T>(T) p, {required String n}) {}

void f(F<int> a, (int, {int b}) r) {
  g(a, n: r);
}

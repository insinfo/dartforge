void f(Object? x) {
  if (x case int(foo: var bar)) {
    bar;
  }
}

extension on int {
  int get foo => 0;
}

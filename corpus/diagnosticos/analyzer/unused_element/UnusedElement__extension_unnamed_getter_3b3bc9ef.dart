void f(Object? x) {
  if (x case List<int>(foo: var bar)) {
    bar;
  }
}

extension<T> on List<T> {
  T get foo => throw 0;
}

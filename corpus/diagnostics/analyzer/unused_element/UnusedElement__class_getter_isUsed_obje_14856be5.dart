void f(Object? x) {
  if (x case A<int>(_foo: var bar)) {
    bar;
  }
}

class A<T> {
  T get _foo => throw 0;
}
